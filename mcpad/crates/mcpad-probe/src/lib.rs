//! `mcpad-probe` — контракт проб.
//!
//! Проб — это источник данных конкретной платформы. Он обязан:
//!
//! * объявить свои [`Capability`];
//! * для каждой заявленной возможности сказать, полная она или частичная
//!   ([`Support`]);
//! * уметь собрать [`Snapshot`], а там, где ОС не даёт данных, — поставить
//!   счётчик в состояние «недоступно» **с причиной**, а не с нулём.
//!
//! Один и тот же снимок читают GUI, CLI, MCP и удалённый клиент, поэтому все
//! пробы обязаны отдавать одну и ту же модель.

use mcpad_core::{Error, ModuleLink, ProcessDetails, Snapshot, UnavailableReason};

/// Целевая платформа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Windows,
    Linux,
    MacOs,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::MacOs => "macos",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Возможность, которую может предоставить платформа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Список процессов.
    ProcessList,
    /// Накопительное время CPU.
    CpuTime,
    /// Доля CPU в процентах.
    CpuPercent,
    /// Working set процесса.
    MemoryWorkingSet,
    /// Приватная память процесса.
    MemoryPrivate,
    /// Число потоков.
    ThreadCount,
    /// Число дескрипторов.
    HandleCount,
    /// Счётчики ввода-вывода.
    IoCounters,
    /// Командная строка.
    CommandLine,
    /// Путь к исполняемому файлу.
    ExePath,
    /// Загрузка CPU в масштабе системы.
    SystemCpu,
    /// Память в масштабе системы.
    SystemMemory,
    /// Температуры и прочие датчики.
    Sensors,
    /// Управление охлаждением.
    FanControl,
    /// Сведения об устройствах GPU.
    GpuDevice,
    /// Видеопамять по процессам.
    GpuPerProcess,
    /// Активность движков GPU.
    GpuEngines,
    /// Список служб.
    Services,
    /// Сетевые соединения.
    NetworkConnections,
    /// Активность файлов по процессам.
    FileActivity,
    /// Стек потока.
    ProcessStackTrace,
    /// Загруженные модули.
    ProcessModules,
    /// Открытые дескрипторы.
    ProcessHandles,
    /// Карта памяти процесса.
    MemoryMap,
    /// Токен и права.
    SecurityToken,
    /// Контейнеры.
    Containers,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::ProcessList => "process_list",
            Capability::CpuTime => "cpu_time",
            Capability::CpuPercent => "cpu_percent",
            Capability::MemoryWorkingSet => "memory_working_set",
            Capability::MemoryPrivate => "memory_private",
            Capability::ThreadCount => "thread_count",
            Capability::HandleCount => "handle_count",
            Capability::IoCounters => "io_counters",
            Capability::CommandLine => "command_line",
            Capability::ExePath => "exe_path",
            Capability::SystemCpu => "system_cpu",
            Capability::SystemMemory => "system_memory",
            Capability::Sensors => "sensors",
            Capability::FanControl => "fan_control",
            Capability::GpuDevice => "gpu_device",
            Capability::GpuPerProcess => "gpu_per_process",
            Capability::GpuEngines => "gpu_engines",
            Capability::Services => "services",
            Capability::NetworkConnections => "network_connections",
            Capability::FileActivity => "file_activity",
            Capability::ProcessStackTrace => "process_stack_trace",
            Capability::ProcessModules => "process_modules",
            Capability::ProcessHandles => "process_handles",
            Capability::MemoryMap => "memory_map",
            Capability::SecurityToken => "security_token",
            Capability::Containers => "containers",
        }
    }

    /// Все возможности — для полного перечня в отчёте о возможностях.
    pub fn all() -> &'static [Capability] {
        use Capability::*;
        &[
            ProcessList,
            CpuTime,
            CpuPercent,
            MemoryWorkingSet,
            MemoryPrivate,
            ThreadCount,
            HandleCount,
            IoCounters,
            CommandLine,
            ExePath,
            SystemCpu,
            SystemMemory,
            Sensors,
            FanControl,
            GpuDevice,
            GpuPerProcess,
            GpuEngines,
            Services,
            NetworkConnections,
            FileActivity,
            ProcessStackTrace,
            ProcessModules,
            ProcessHandles,
            MemoryMap,
            SecurityToken,
            Containers,
        ]
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Уровень поддержки возможности.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Поддерживается полностью.
    Full,
    /// Поддерживается частично (например, только для своих процессов).
    Partial,
    /// Не поддерживается, и известно почему.
    Unsupported(UnavailableReason),
}

impl Support {
    pub fn is_available(self) -> bool {
        !matches!(self, Support::Unsupported(_))
    }
}

/// Источник данных платформы.
pub trait PlatformProbe: Send + Sync {
    /// Платформа.
    fn platform(&self) -> Platform;

    /// Возможности, которые проб реализует полностью.
    fn capabilities(&self) -> &'static [Capability];

    /// Поддержка конкретной возможности.
    fn support(&self, capability: Capability) -> Support {
        if self.capabilities().contains(&capability) {
            Support::Full
        } else {
            Support::Unsupported(UnavailableReason::NotSupported)
        }
    }

    /// Собирает сырой снимок: накопительные счётчики без производных величин.
    ///
    /// Проценты CPU считает сэмплер — из дельт между соседними снимками, — чтобы
    /// правило расчёта было одно на всех платформах.
    fn collect(&self) -> Result<Snapshot, Error>;

    /// Подробности одного процесса — дорогой путь, по запросу.
    fn process_details(&self, _pid: u32) -> Result<ProcessDetails, Error> {
        Err(Error::Unsupported("process details".to_string()))
    }

    /// Модули, загруженные в процесс.
    ///
    /// Перечисление модулей есть не на каждой платформе и не для каждого
    /// процесса. Поэтому по умолчанию проб отвечает отказом с причиной, а не
    /// пустым списком: пустой список означал бы «модулей нет», что неверно.
    fn module_links(&self, _pid: u32) -> Result<Vec<ModuleLink>, Error> {
        Err(Error::Unsupported(
            "перечисление модулей для этой платформы не реализовано".to_string(),
        ))
    }

    /// Явные ограничения платформы: возможность и причина недоступности.
    fn limitations(&self) -> Vec<(Capability, UnavailableReason)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl PlatformProbe for Dummy {
        fn platform(&self) -> Platform {
            Platform::Windows
        }

        fn capabilities(&self) -> &'static [Capability] {
            &[Capability::ProcessList, Capability::CpuTime]
        }

        fn collect(&self) -> Result<Snapshot, Error> {
            Ok(Snapshot::new(mcpad_core::SystemInfo::default(), Vec::new()))
        }
    }

    #[test]
    fn capability_names_are_unique() {
        let mut names: Vec<&str> = Capability::all().iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn unsupported_capability_reports_reason() {
        let probe = Dummy;
        assert_eq!(
            probe.support(Capability::GpuPerProcess),
            Support::Unsupported(UnavailableReason::NotSupported)
        );
        assert!(probe.support(Capability::ProcessList).is_available());
    }
}
