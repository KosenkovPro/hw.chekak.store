//! Вызовы `libproc` — слой FFI. Собирается только под macOS.

use std::ffi::c_void;

use mcpad_core::UnavailableReason;
use mcpad_core::{Counter, ProcessDetails, ProcessInfo, ProcessKey, Snapshot, Source, SystemInfo};

use crate::parse::{self, BsdInfo, TaskInfo};

/// `PROC_ALL_PIDS` — все процессы.
const PROC_ALL_PIDS: u32 = 1;
/// `PROC_PIDLISTFDS`.
const PROC_PIDLISTFDS: i32 = 1;
/// `PROC_PIDT_SHORTBSDINFO`.
const PROC_PIDT_SHORTBSDINFO: i32 = 3;
/// `PROC_PIDTASKINFO`.
const PROC_PIDTASKINFO: i32 = 4;

#[link(name = "proc")]
unsafe extern "C" {
    fn proc_listpids(type_: u32, typeinfo: u32, buffer: *mut c_void, buffersize: i32) -> i32;

    fn proc_pidinfo(
        pid: libc::c_int,
        flavor: i32,
        arg: u64,
        buffer: *mut c_void,
        buffersize: i32,
    ) -> i32;

    fn proc_pidpath(pid: libc::c_int, buffer: *mut c_void, buffersize: u32) -> i32;
}

/// Собирает снимок системы.
pub fn collect_snapshot() -> Result<Snapshot, mcpad_core::Error> {
    let boot = boot_time_unix();
    let processes = collect_processes(boot);
    let system = collect_system(boot)?;
    Ok(Snapshot::new(system, processes))
}

/// Перечисляет PID через `proc_listpids`.
fn list_pids() -> Vec<u32> {
    let mut buffer = vec![0u32; 16 * 1024];
    // SAFETY: буфер выделен и живёт до конца вызова, длина в байтах соответствует
    // фактическому размеру буфера, `PROC_ALL_PIDS` не требует дополнительных данных.
    let written = unsafe {
        proc_listpids(
            PROC_ALL_PIDS,
            0,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * std::mem::size_of::<u32>()) as i32,
        )
    };
    if written <= 0 {
        return Vec::new();
    }
    let count = (written as usize / std::mem::size_of::<u32>()).min(buffer.len());
    buffer[..count]
        .iter()
        .copied()
        .filter(|pid| *pid > 0)
        .collect()
}

/// Читает `proc_bsdinfo` процесса.
fn read_bsdinfo(pid: u32) -> Option<BsdInfo> {
    let mut buffer = vec![0u8; 0x100];
    // SAFETY: буфер выделен, длина передаётся явно, `proc_pidinfo` пишет
    // не больше указанного размера.
    let written = unsafe {
        proc_pidinfo(
            pid as libc::c_int,
            PROC_PIDT_SHORTBSDINFO,
            0,
            buffer.as_mut_ptr().cast(),
            buffer.len() as i32,
        )
    };
    if written <= 0 {
        return None;
    }
    parse::parse_bsdinfo(&buffer[..written as usize])
}

/// Читает `proc_taskinfo` процесса.
fn read_taskinfo(pid: u32) -> Option<TaskInfo> {
    let mut buffer = vec![0u8; 0x100];
    // SAFETY: буфер выделен, длина передаётся явно, `proc_pidinfo` пишет
    // не больше указанного размера.
    let written = unsafe {
        proc_pidinfo(
            pid as libc::c_int,
            PROC_PIDTASKINFO,
            0,
            buffer.as_mut_ptr().cast(),
            buffer.len() as i32,
        )
    };
    if written <= 0 {
        return None;
    }
    parse::parse_taskinfo(&buffer[..written as usize])
}

fn collect_processes(boot: Option<u64>) -> Vec<ProcessInfo> {
    list_pids()
        .into_iter()
        .filter_map(|pid| {
            let bsd = read_bsdinfo(pid)?;
            let task = read_taskinfo(pid);
            Some(build_process_info(pid, &bsd, task.as_ref(), boot))
        })
        .collect()
}

fn build_process_info(
    pid: u32,
    bsd: &BsdInfo,
    task: Option<&TaskInfo>,
    boot: Option<u64>,
) -> ProcessInfo {
    let start_time_unix = (bsd.start_seconds > 0).then_some(bsd.start_seconds);
    let key = ProcessKey::new(pid, start_time_unix, boot);
    let mut info = ProcessInfo::new(key, bsd.name.clone());

    info.parent_pid = (bsd.ppid != 0).then_some(bsd.ppid);

    if let Some(task) = task {
        let cpu = task.total_user.saturating_add(task.total_system);
        info.cpu_time_total = Counter::measured(cpu, Source::MacOsLibproc);
        info.memory_working_set = Counter::measured(task.resident_size, Source::MacOsLibproc);
        info.memory_private = Counter::estimated(task.virtual_size, Source::MacOsLibproc);
        info.thread_count = match task.thread_count {
            value if value >= 0 => Counter::measured(value as u32, Source::MacOsLibproc),
            _ => Counter::unavailable(UnavailableReason::NotMeasured),
        };
    } else {
        info.cpu_time_total = Counter::unavailable(UnavailableReason::ReadFailed);
        info.memory_working_set = Counter::unavailable(UnavailableReason::ReadFailed);
        info.memory_private = Counter::unavailable(UnavailableReason::ReadFailed);
        info.thread_count = Counter::unavailable(UnavailableReason::ReadFailed);
    }

    info.handle_count = match count_handles(pid) {
        Some(count) => Counter::measured(count, Source::MacOsLibproc),
        None => Counter::unavailable(UnavailableReason::RequiresElevation),
    };

    info
}

/// Считает открытые дескрипторы через `PROC_PIDLISTFDS`.
fn count_handles(pid: u32) -> Option<u32> {
    let mut buffer = vec![0u8; 16 * 1024];
    // SAFETY: буфер выделен, длина передаётся явно, `proc_pidinfo` пишет
    // не больше указанного размера.
    let written = unsafe {
        proc_pidinfo(
            pid as libc::c_int,
            PROC_PIDLISTFDS,
            0,
            buffer.as_mut_ptr().cast(),
            buffer.len() as i32,
        )
    };
    if written <= 0 {
        return None;
    }
    Some((written as usize / 4) as u32)
}

/// Сводка по системе.
fn collect_system(boot: Option<u64>) -> Result<SystemInfo, mcpad_core::Error> {
    // SAFETY: `sysconf` безопасен при любом корректном имени параметра;
    // возвращает -1 при ошибке, что проверяется ниже.
    let logical_cpus = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    let logical_cpus = if logical_cpus > 0 {
        logical_cpus as u32
    } else {
        1
    };

    Ok(SystemInfo {
        hostname: hostname(),
        os_name: "macOS".to_string(),
        os_version: os_version(),
        arch: arch(),
        logical_cpus,
        boot_time_unix: boot,
        cpu_percent: Counter::unavailable(UnavailableReason::NotMeasured),
        cpu_time_total: Counter::unavailable(UnavailableReason::NotMeasured),
        memory_total: Counter::unavailable(UnavailableReason::NotMeasured),
        memory_available: Counter::unavailable(UnavailableReason::NotMeasured),
        pagefile_total: Counter::unavailable(UnavailableReason::NotMeasured),
    })
}

/// Время загрузки: `sysctl KERN_BOOTTIME`.
fn boot_time_unix() -> Option<u64> {
    // SAFETY: `timeval` — POD-структура, нулевая инициализация допустима.
    let mut value: libc::timeval = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::timeval>();
    let mut mib = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    // SAFETY: указатели ссылаются на живые переменные, длина `size` передаётся явно.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            &mut value as *mut _ as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return None;
    }
    Some(value.tv_sec as u64)
}

fn hostname() -> Option<String> {
    let mut buffer = [0u8; 256];
    let mut size = buffer.len();
    // SAFETY: буфер выделен на стеке, длина передаётся явно и соответствует буферу.
    let result = unsafe { libc::gethostname(buffer.as_mut_ptr() as *mut libc::c_char, size) };
    if result != 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf8_lossy(&buffer[..end]).to_string())
}

/// Версия macOS из `sysctl kern.osrelease`.
fn os_version() -> String {
    let mut buffer = [0u8; 64];
    let mut size = buffer.len();
    let mut mib = [libc::CTL_KERN, libc::KERN_OSRELEASE];
    // SAFETY: указатели ссылаются на живые переменные, длина `size` передаётся явно.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            buffer.as_mut_ptr() as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return "unknown".to_string();
    }
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).to_string()
}

fn arch() -> String {
    if cfg!(target_arch = "aarch64") {
        "aarch64".to_string()
    } else if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Подробности процесса: путь к образу и владелец.
pub fn query_details(pid: u32) -> ProcessDetails {
    let mut details = ProcessDetails {
        key: ProcessKey::from_pid(pid),
        exe_path: None,
        command_line: None,
        user: None,
    };

    let mut buffer = vec![0u8; 4096];
    // SAFETY: буфер выделен и живёт до конца вызова, длина в байтах соответствует
    // фактическому размеру буфера.
    let written = unsafe {
        proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if written > 0 {
        let end = buffer[..written as usize]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(written as usize);
        let path = String::from_utf8_lossy(&buffer[..end]).to_string();
        if !path.is_empty() {
            details.exe_path = Some(path);
        }
    }

    if let Some(bsd) = read_bsdinfo(pid) {
        details.user = user_name(bsd.uid);
    }

    details
}

/// Имя пользователя по UID.
fn user_name(uid: u32) -> Option<String> {
    // SAFETY: `getpwuid` возвращает указатель на статическую запись или null;
    // при null выходим, иначе читаем NUL- terminated `pw_name`.
    unsafe {
        let entry = libc::getpwuid(uid);
        if entry.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*entry).pw_name);
        Some(name.to_string_lossy().to_string())
    }
}
