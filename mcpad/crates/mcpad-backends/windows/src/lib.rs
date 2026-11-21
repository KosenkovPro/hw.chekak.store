//! `mcpad-backends-windows` — источник данных для Windows.
//!
//! Быстрый путь (`collect`) — один вызов `NtQuerySystemInformation`:
//! имена, родители, время CPU, память, потоки, дескрипторы, ввод-вывод.
//!
//! Подробный путь (`process_details`) — открытие процесса: путь к образу,
//! командная строка, владелец. Он вызывается по запросу пользователя.
//!
//! Крейт собирается только под Windows: привязки Win32 на других системах
//! не существуют. В workspace он остаётся членом, поэтому вне Windows он
//! собирается пустым — иначе падала бы сборка всего workspace в CI.

#![cfg(windows)]

mod control;
mod details;
mod modules;
mod parse;
mod system;

use mcpad_core::{
    Counter, Error, ModuleLink, Priority, ProcessDetails, Snapshot, Source, SystemInfo,
    UnavailableReason,
};
use mcpad_probe::{Capability, Platform, PlatformProbe, Support};
use windows::Win32::Foundation::FILETIME;
// Раскладка этих функций по модулям в привязках windows 0.62 не совпадает
// с привычной: счётчики процессоров и время системы — в Threading, память —
// в SystemInformation, имя компьютера — в WindowsProgramming.
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::{
    ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetSystemTimes,
};
use windows::Win32::System::WindowsProgramming::GetComputerNameW;
use windows::core::PWSTR;

/// Проб Windows.
pub struct WindowsProbe;

impl PlatformProbe for WindowsProbe {
    fn platform(&self) -> Platform {
        Platform::Windows
    }

    fn capabilities(&self) -> &'static [Capability] {
        &[
            Capability::ProcessList,
            Capability::CpuTime,
            Capability::CpuPercent,
            Capability::MemoryWorkingSet,
            Capability::MemoryPrivate,
            Capability::ThreadCount,
            Capability::HandleCount,
            Capability::IoCounters,
            Capability::ExePath,
            Capability::CommandLine,
            Capability::SystemCpu,
            Capability::SystemMemory,
            Capability::SecurityToken,
            Capability::ProcessControl,
            Capability::ProcessPriority,
        ]
    }

    fn support(&self, capability: Capability) -> Support {
        match capability {
            Capability::CommandLine | Capability::SecurityToken => {
                // Нужны PROCESS_VM_READ и открытие токена: у части процессов
                // прав не хватает, и это видно как «нет данных», а не как ноль.
                Support::Partial
            }
            Capability::ProcessModules => {
                // Список модулей доступен для процессов, которые удалось
                // открыть; у защищённых процессов доступа нет.
                Support::Partial
            }
            Capability::ProcessControl | Capability::ProcessPriority => {
                // Действия доступны только для процессов, на которые у
                // пользователя есть права.
                Support::Partial
            }
            Capability::GpuPerProcess
            | Capability::GpuDevice
            | Capability::GpuEngines
            | Capability::Sensors
            | Capability::FanControl
            | Capability::Services
            | Capability::NetworkConnections
            | Capability::FileActivity
            | Capability::ProcessStackTrace
            | Capability::ProcessHandles
            | Capability::MemoryMap
            | Capability::Containers => Support::Unsupported(UnavailableReason::NotSupported),
            _ => Support::Full,
        }
    }

    fn collect(&self) -> Result<Snapshot, Error> {
        let raw = system::query_processes()?;
        let boot_id = boot_time_unix();
        let processes = raw
            .into_iter()
            .map(|entry| system::into_process_info(entry, boot_id))
            .collect();

        Ok(Snapshot::new(system_info(boot_id)?, processes))
    }

    fn process_details(&self, pid: u32) -> Result<ProcessDetails, Error> {
        Ok(details::query_details(pid))
    }

    fn module_links(&self, pid: u32) -> Result<Vec<ModuleLink>, Error> {
        modules::module_links(pid)
    }

    fn terminate_process(&self, pid: u32) -> Result<(), Error> {
        control::terminate(pid)
    }

    fn set_priority(&self, pid: u32, priority: Priority) -> Result<(), Error> {
        control::set_priority(pid, priority)
    }
}

/// Проб для текущей платформы.
pub fn platform_probe() -> Result<Box<dyn PlatformProbe>, Error> {
    Ok(Box::new(WindowsProbe))
}

/// Время загрузки системы в секундах Unix.
fn boot_time_unix() -> Option<u64> {
    // SAFETY: функция без аргументов и без указателей, возвращает u64.
    let uptime_ms = unsafe { GetTickCount64() };
    let now_ms = mcpad_core::snapshot::now_unix_ms();
    Some(now_ms.saturating_sub(uptime_ms) / 1000)
}

/// Сводка по системе.
fn system_info(boot_id: Option<u64>) -> Result<SystemInfo, Error> {
    // SAFETY: функция без аргументов и без указателей, возвращает u32.
    let logical_cpus = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };

    let (idle, kernel, user) = system_times()?;
    // kernel включает idle, поэтому «занятое» время — это kernel + user - idle.
    let busy_100ns = kernel.saturating_sub(idle).saturating_add(user).max(0);
    let cpu_time_total = Counter::measured(
        (busy_100ns as u64).saturating_mul(100),
        Source::WindowsPerformanceCounter,
    );

    let (total, available, pagefile) = memory_status()?;

    Ok(SystemInfo {
        hostname: computer_name(),
        os_name: "Windows".to_string(),
        os_version: os_version(),
        arch: arch(),
        logical_cpus,
        boot_time_unix: boot_id,
        cpu_percent: Counter::unavailable(UnavailableReason::NotMeasured),
        cpu_time_total,
        memory_total: Counter::measured(total, Source::WindowsPerformanceCounter),
        memory_available: Counter::measured(available, Source::WindowsPerformanceCounter),
        pagefile_total: Counter::measured(pagefile, Source::WindowsPerformanceCounter),
    })
}

fn system_times() -> Result<(i64, i64, i64), Error> {
    // SAFETY: FILETIME — POD-структура из двух целых, нулевая инициализация допустима.
    let mut idle: FILETIME = unsafe { std::mem::zeroed() };
    // SAFETY: FILETIME — POD-структура из двух целых, нулевая инициализация допустима.
    let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
    // SAFETY: FILETIME — POD-структура из двух целых, нулевая инициализация допустима.
    let mut user: FILETIME = unsafe { std::mem::zeroed() };
    // SAFETY: указатели ссылаются на живые переменные выше, вызов их только заполняет.
    unsafe {
        GetSystemTimes(
            Some(&mut idle as *mut FILETIME),
            Some(&mut kernel as *mut FILETIME),
            Some(&mut user as *mut FILETIME),
        )
    }
    .map_err(|e| Error::Io(format!("GetSystemTimes: {e}")))?;
    Ok((
        filetime_to_i64(idle),
        filetime_to_i64(kernel),
        filetime_to_i64(user),
    ))
}

fn memory_status() -> Result<(u64, u64, u64), Error> {
    // SAFETY: MEMORYSTATUSEX — POD-структура, нулевая инициализация допустима.
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: указатель ссылается на живую структуру с корректной длиной.
    unsafe { GlobalMemoryStatusEx(&mut status) }
        .map_err(|e| Error::Io(format!("GlobalMemoryStatusEx: {e}")))?;
    Ok((
        status.ullTotalPhys,
        status.ullAvailPhys,
        status.ullTotalPageFile,
    ))
}

fn computer_name() -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut size = buffer.len() as u32;
    // SAFETY: буфер выделен на стеке, длина передаётся явно и соответствует буферу.
    unsafe { GetComputerNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size) }.ok()?;
    Some(String::from_utf16_lossy(&buffer[..size as usize]))
        .map(|name| name.trim_end_matches('\0').to_string())
        .filter(|name| !name.is_empty())
}

fn os_version() -> String {
    let (major, minor, build) = windows_version();
    format!("{major}.{minor}.{build}")
}

/// Версия Windows: старшая, младшая части и номер сборки.
fn windows_version() -> (u32, u32, u32) {
    #[repr(C)]
    struct RtlOsVersionInfoExW {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
    }

    unsafe extern "system" {
        fn RtlGetVersion(version: *mut core::ffi::c_void) -> i32;
    }

    let mut version = RtlOsVersionInfoExW {
        size: std::mem::size_of::<RtlOsVersionInfoExW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        csd: [0; 128],
    };
    // SAFETY: указатель ссылается на живую структуру версии с заполненным полем size.
    let status = unsafe { RtlGetVersion(&mut version as *mut _ as *mut core::ffi::c_void) };
    if status == 0 {
        (version.major, version.minor, version.build)
    } else {
        (0, 0, 0)
    }
}

fn arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "aarch64".to_string()
    } else if cfg!(target_arch = "x86") {
        "x86".to_string()
    } else {
        "unknown".to_string()
    }
}

fn filetime_to_i64(value: FILETIME) -> i64 {
    ((value.dwHighDateTime as i64) << 32) | value.dwLowDateTime as i64
}
