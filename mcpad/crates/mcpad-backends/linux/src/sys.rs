//! Чтение `/proc` — слой ввода-вывода. Собирается только под Linux.

use std::fs;

use mcpad_core::UnavailableReason;
use mcpad_core::{
    Counter, ModuleLink, ProcessDetails, ProcessInfo, ProcessKey, Snapshot, Source, SystemInfo,
};

use crate::parse::{self, CpuTimes, ProcStat};

/// Тиков в секунду (`SC_CLK_TCK`).
fn clock_ticks_per_second() -> u64 {
    // SAFETY: `sysconf` безопасен при любом корректном имени параметра;
    // возвращает -1 при ошибке, что проверяется ниже.
    let value = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if value > 0 { value as u64 } else { 100 }
}

/// Число онлайн-процессоров.
fn online_cpus() -> u32 {
    // SAFETY: `sysconf` безопасен при любом корректном имени параметра;
    // возвращает -1 при ошибке, что проверяется ниже.
    let value = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if value > 0 { value as u32 } else { 1 }
}

/// Идентификатор загрузки: хеш `/proc/sys/kernel/random/boot_id`.
fn boot_id() -> Option<u64> {
    let text = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    Some(parse::hash_boot_id(&text))
}

/// Собирает снимок системы.
pub fn collect_snapshot() -> Result<Snapshot, mcpad_core::Error> {
    let boot = boot_id();
    let processes = collect_processes(boot);
    let system = collect_system(boot)?;
    Ok(Snapshot::new(system, processes))
}

/// Собирает список процессов из `/proc`.
fn collect_processes(boot: Option<u64>) -> Vec<ProcessInfo> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    let mut pids: Vec<u32> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .collect();
    pids.sort_unstable();

    let ticks_per_second = clock_ticks_per_second();
    let boot_seconds = boot_time_unix().unwrap_or(0);

    pids.into_iter()
        .filter_map(|pid| {
            let stat = read_stat(pid)?;
            let status = read_status(pid);
            Some(build_process_info(
                pid,
                &stat,
                &status,
                ticks_per_second,
                boot,
                boot_seconds,
            ))
        })
        .collect()
}

fn build_process_info(
    pid: u32,
    stat: &ProcStat,
    status: &parse::ProcStatus,
    ticks_per_second: u64,
    boot: Option<u64>,
    boot_seconds: u64,
) -> ProcessInfo {
    let start_time_unix = if stat.start_ticks > 0 {
        Some(boot_seconds + stat.start_ticks / ticks_per_second)
    } else {
        None
    };

    let key = ProcessKey::new(pid, start_time_unix, boot);
    let mut info = ProcessInfo::new(key, stat.comm.clone());
    info.parent_pid = (stat.ppid != 0).then_some(stat.ppid);

    let cpu_ticks = stat.utime_ticks.saturating_add(stat.stime_ticks);
    info.cpu_time_total = Counter::measured(
        parse::ticks_to_nanos(cpu_ticks, ticks_per_second),
        Source::LinuxProcFs,
    );

    let working_set = status
        .vm_rss_bytes
        .or_else(|| Some(stat.rss_pages.saturating_mul(page_size())));
    info.memory_working_set = match working_set {
        Some(value) => Counter::measured(value, Source::LinuxProcFs),
        None => Counter::unavailable(UnavailableReason::NotMeasured),
    };

    info.memory_private = match status.vm_size_bytes {
        Some(value) => Counter::inferred(value, Source::LinuxProcFs),
        None => Counter::unavailable(UnavailableReason::NotMeasured),
    };

    info.thread_count = match status.threads.or(Some(stat.threads)) {
        Some(value) => Counter::measured(value, Source::LinuxProcFs),
        None => Counter::unavailable(UnavailableReason::NotMeasured),
    };

    info.handle_count = match count_handles(pid) {
        Some(count) => Counter::measured(count, Source::LinuxProcFs),
        None => Counter::unavailable(UnavailableReason::RequiresElevation),
    };

    if let Some((read, write)) = read_io(pid) {
        info.io_read_bytes = Counter::measured(read, Source::LinuxProcFs);
        info.io_write_bytes = Counter::measured(write, Source::LinuxProcFs);
    } else {
        info.io_read_bytes = Counter::unavailable(UnavailableReason::RequiresElevation);
        info.io_write_bytes = Counter::unavailable(UnavailableReason::RequiresElevation);
    }

    info
}

/// Размер страницы в байтах.
fn page_size() -> u64 {
    // SAFETY: `sysconf` безопасен при любом корректном имени параметра;
    // возвращает -1 при ошибке, что проверяется ниже.
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if value > 0 { value as u64 } else { 4096 }
}

/// Время загрузки в секундах Unix.
fn boot_time_unix() -> Option<u64> {
    let uptime = parse::parse_uptime(&fs::read_to_string("/proc/uptime").ok()?)?;
    let now = mcpad_core::snapshot::now_unix_ms() as f64 / 1000.0;
    Some((now - uptime).max(0.0) as u64)
}

fn read_stat(pid: u32) -> Option<ProcStat> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse::parse_proc_stat(&text)
}

fn read_status(pid: u32) -> parse::ProcStatus {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .map(|text| parse::parse_proc_status(&text))
        .unwrap_or_default()
}

/// Считает открытые дескрипторы процесса.
fn count_handles(pid: u32) -> Option<u32> {
    let entries = fs::read_dir(format!("/proc/{pid}/fd")).ok()?;
    Some(entries.flatten().count() as u32)
}

/// Читает счётчики ввода-вывода процесса (нужны права владельца или root).
fn read_io(pid: u32) -> Option<(u64, u64)> {
    let text = fs::read_to_string(format!("/proc/{pid}/io")).ok()?;
    let mut read = None;
    let mut write = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let number: u64 = value.trim().parse().ok()?;
        match key {
            "rchar" => read = Some(number),
            "wchar" => write = Some(number),
            _ => {}
        }
    }
    Some((read?, write?))
}

/// Сводка по системе.
fn collect_system(boot: Option<u64>) -> Result<SystemInfo, mcpad_core::Error> {
    let stat = fs::read_to_string("/proc/stat")
        .map_err(|e| mcpad_core::Error::Io(format!("/proc/stat: {e}")))?;
    let cpu: Option<CpuTimes> = stat.lines().find_map(parse::parse_cpu_line);

    let ticks_per_second = clock_ticks_per_second();
    let cpu_time_total = match cpu {
        Some(times) => Counter::measured(
            parse::ticks_to_nanos(times.busy_ticks, ticks_per_second),
            Source::LinuxProcFs,
        ),
        None => Counter::unavailable(UnavailableReason::ReadFailed),
    };

    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let memory = parse::parse_meminfo(&meminfo);

    Ok(SystemInfo {
        hostname: fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .map(|name| name.trim().to_string()),
        os_name: "Linux".to_string(),
        os_version: kernel_version(),
        arch: arch(),
        logical_cpus: online_cpus(),
        boot_time_unix: boot_time_unix().or(boot),
        cpu_percent: Counter::unavailable(UnavailableReason::NotMeasured),
        cpu_time_total,
        memory_total: counter(memory.mem_total_bytes),
        memory_available: counter(memory.mem_available_bytes),
        pagefile_total: counter(memory.swap_total_bytes),
    })
}

fn counter(value: Option<u64>) -> Counter<u64> {
    match value {
        Some(value) => Counter::measured(value, Source::LinuxProcFs),
        None => Counter::unavailable(UnavailableReason::ReadFailed),
    }
}

/// Версия ядра из `uname`.
fn kernel_version() -> String {
    // SAFETY: `utsname` — POD-структура, нулевая инициализация допустима,
    // указатель передаётся в `uname`, который заполняет её целиком.
    unsafe {
        let mut buf: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut buf) != 0 {
            return "unknown".to_string();
        }
        let release = bytes_to_string(buf.release.as_ptr());
        let version = bytes_to_string(buf.version.as_ptr());
        format!("{release} ({version})")
    }
}

fn bytes_to_string(pointer: *const libc::c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: указатель ненулевой и указывает на NUL- terminated строку из `uname`.
    unsafe {
        std::ffi::CStr::from_ptr(pointer)
            .to_string_lossy()
            .trim()
            .to_string()
    }
}

fn arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "aarch64".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Модули процесса: файлы, размеченные в его памяти.
///
/// Источник — `/proc/<pid>/maps`. Это не полный перечень загруженных модулей
/// (там нет, например, `memfd`-образов), поэтому путь к модулю — это то, что
/// видно, а не «официальные» данные загрузчика.
pub fn module_links(pid: u32) -> Result<Vec<ModuleLink>, mcpad_core::Error> {
    let text = fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|error| mcpad_core::Error::Io(format!("/proc/{pid}/maps: {error}")))?;

    let process_label = fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("pid {pid}"));

    Ok(parse::parse_maps(&text)
        .into_iter()
        .map(|file| {
            let label = ModuleLink::file_name(&file.path).to_string();
            ModuleLink::new(pid, process_label.clone(), label, Some(file.path))
        })
        .collect())
}

/// Подробности процесса: путь к образу, командная строка, владелец.
pub fn query_details(pid: u32) -> ProcessDetails {
    let mut details = ProcessDetails {
        key: ProcessKey::from_pid(pid),
        exe_path: None,
        command_line: None,
        user: None,
    };

    if let Ok(bytes) = fs::read(format!("/proc/{pid}/cmdline")) {
        details.command_line = parse::parse_cmdline(&bytes);
    }

    if let Ok(path) = fs::read_link(format!("/proc/{pid}/exe")) {
        details.exe_path = Some(path.to_string_lossy().to_string());
    }

    let status = read_status(pid);
    if let Some(uid) = status.uid {
        details.user = user_name(uid);
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
