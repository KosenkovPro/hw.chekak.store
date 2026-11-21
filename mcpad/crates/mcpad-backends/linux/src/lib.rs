//! `mcpad-backends-linux` — источник данных для Linux.
//!
//! Быстрый путь — чтение `/proc`: список процессов, CPU, память, потоки,
//! дескрипторы. Подробный путь (`process_details`) — путь к образу, командная
//! строка и владелец.
//!
//! Модуль `parse` собирается на любой платформе: так парсеры `/proc` покрыты
//! тестами даже на Windows CI. Модуль `sys` — только под Linux.

pub mod parse;

#[cfg(target_os = "linux")]
mod sys;

use mcpad_core::Error;

#[cfg(target_os = "linux")]
use mcpad_core::ModuleLink;
#[cfg(target_os = "linux")]
use mcpad_core::ProcessDetails;
#[cfg(target_os = "linux")]
use mcpad_core::Snapshot;
#[cfg(target_os = "linux")]
use mcpad_probe::{Capability, Platform, PlatformProbe, Support};

/// Проб Linux.
#[cfg(target_os = "linux")]
pub struct LinuxProbe;

#[cfg(target_os = "linux")]
impl PlatformProbe for LinuxProbe {
    fn platform(&self) -> Platform {
        Platform::Linux
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
            Capability::CommandLine,
            Capability::ExePath,
            Capability::SystemCpu,
            Capability::SystemMemory,
        ]
    }

    fn support(&self, capability: Capability) -> Support {
        match capability {
            // Счётчики ввода-вывода читаются из /proc/<pid>/io, доступного
            // только владельцу процесса или root.
            Capability::IoCounters => Support::Partial,
            // Модули берутся из /proc/<pid>/maps: список видимых файлов, а не
            // перечень загрузчика, и для чужих процессов может быть закрыт.
            Capability::ProcessModules => Support::Partial,
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

    fn module_links(&self, pid: u32) -> Result<Vec<ModuleLink>, Error> {
        sys::module_links(pid)
    }
}

/// Проб для текущей платформы.
pub fn platform_probe() -> Result<Box<dyn mcpad_probe::PlatformProbe>, Error> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(LinuxProbe))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(Error::Unsupported(
            "бэкенд Linux собирается только под Linux".to_string(),
        ))
    }
}
