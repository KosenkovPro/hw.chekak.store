//! `mcpad-backends-macos` — источник данных для macOS.
//!
//! Быстрый путь — `libproc`: список процессов, CPU, память, потоки.
//! Подробный путь — путь к образу и владелец.
//!
//! Командная строка на macOS требует `sysctl KERN_PROCARGS2` с правами root,
//! поэтому она помечена как недоступная, а не выдумывается.

pub mod parse;

#[cfg(target_os = "macos")]
mod control;
#[cfg(target_os = "macos")]
mod sys;

use mcpad_core::Error;

#[cfg(target_os = "macos")]
use mcpad_core::Priority;
#[cfg(target_os = "macos")]
use mcpad_core::ProcessDetails;
#[cfg(target_os = "macos")]
use mcpad_core::Snapshot;
#[cfg(target_os = "macos")]
use mcpad_probe::{Capability, Platform, PlatformProbe, Support};

/// Проб macOS.
#[cfg(target_os = "macos")]
pub struct MacOsProbe;

#[cfg(target_os = "macos")]
impl PlatformProbe for MacOsProbe {
    fn platform(&self) -> Platform {
        Platform::MacOs
    }

    fn capabilities(&self) -> &'static [Capability] {
        &[
            Capability::ProcessList,
            Capability::CpuTime,
            Capability::CpuPercent,
            Capability::MemoryWorkingSet,
            Capability::ThreadCount,
            Capability::ExePath,
            Capability::SystemCpu,
            Capability::SystemMemory,
            Capability::ProcessControl,
            Capability::ProcessPriority,
        ]
    }

    fn support(&self, capability: Capability) -> Support {
        match capability {
            // Командная строка на macOS читается только с правами root.
            Capability::CommandLine => Support::Partial,
            Capability::MemoryPrivate => Support::Partial,
            // Завершение и приоритет доступны только для процессов, на
            // которые у пользователя есть права.
            Capability::ProcessControl | Capability::ProcessPriority => Support::Partial,
            Capability::IoCounters
            | Capability::HandleCount
            | Capability::GpuPerProcess
            | Capability::GpuDevice
            | Capability::GpuEngines
            | Capability::Sensors
            | Capability::FanControl
            | Capability::Services
            | Capability::NetworkConnections
            | Capability::FileActivity
            | Capability::ProcessStackTrace
            | Capability::ProcessModules
            | Capability::ProcessHandles
            | Capability::MemoryMap
            | Capability::SecurityToken
            | Capability::Containers => {
                Support::Unsupported(mcpad_core::UnavailableReason::NotSupported)
            }
            _ => Support::Full,
        }
    }

    fn collect(&self) -> Result<Snapshot, Error> {
        sys::collect_snapshot()
    }

    fn process_details(&self, pid: u32) -> Result<ProcessDetails, Error> {
        Ok(sys::query_details(pid))
    }

    fn terminate_process(&self, pid: u32) -> Result<(), Error> {
        control::terminate(pid)
    }

    fn set_priority(&self, pid: u32, priority: Priority) -> Result<(), Error> {
        control::set_priority(pid, priority)
    }
}

/// Проб для текущей платформы.
pub fn platform_probe() -> Result<Box<dyn mcpad_probe::PlatformProbe>, Error> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOsProbe))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(Error::Unsupported(
            "бэкенд macOS собирается только под macOS".to_string(),
        ))
    }
}
