//! Значения с provenance: источник, уверенность и причина недоступности.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Уровень уверенности в значении.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Измерено прибором или системным вызовом.
    Measured,
    /// Выведено из других данных (например, сопоставление счётчиков).
    Inferred,
    /// Оценено по косвенным признакам (метаданные модели, арифметика).
    Estimated,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Measured => "measured",
            Confidence::Inferred => "inferred",
            Confidence::Estimated => "estimated",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Источник данных. Показывает, каким API получено значение.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Windows: счётчики производительности (PDH).
    WindowsPerformanceCounter,
    /// Windows: `NtQuerySystemInformation`.
    WindowsSystemInformation,
    /// Windows: ETW.
    Etw,
    /// Windows: WMI.
    WindowsWmi,
    /// NVIDIA NVML.
    NvidiaNvml,
    /// AMD Display Library (ADL).
    AmdAdl,
    /// AMD SMI.
    AmdSmi,
    /// Intel PMU / performance counters.
    IntelPerformanceCounter,
    /// Linux: DRM fdinfo (amdgpu, i915, xe).
    DrmFdInfo,
    /// Linux: `/proc`.
    LinuxProcFs,
    /// Linux: `sysfs`.
    LinuxSysfs,
    /// Linux: `hwmon`.
    LinuxHwmon,
    /// Linux: cgroup v2.
    LinuxCgroup,
    /// Linux: сокеты через netlink.
    LinuxNetlink,
    /// macOS: `libproc`.
    MacOsLibproc,
    /// macOS: IOKit / IOReport.
    MacOsIokit,
    /// macOS: Metal.
    Metal,
    /// Диспетчер служб ОС (SCM, systemd, launchd).
    ServiceManager,
    /// Внутренний расчёт MCPad (дельты, тренды, оценки).
    Derived,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::WindowsPerformanceCounter => "windows_performance_counter",
            Source::WindowsSystemInformation => "windows_system_information",
            Source::Etw => "etw",
            Source::WindowsWmi => "windows_wmi",
            Source::NvidiaNvml => "nvidia_nvml",
            Source::AmdAdl => "amd_adl",
            Source::AmdSmi => "amd_smi",
            Source::IntelPerformanceCounter => "intel_performance_counter",
            Source::DrmFdInfo => "drm_fdinfo",
            Source::LinuxProcFs => "linux_procfs",
            Source::LinuxSysfs => "linux_sysfs",
            Source::LinuxHwmon => "linux_hwmon",
            Source::LinuxCgroup => "linux_cgroup",
            Source::LinuxNetlink => "linux_netlink",
            Source::MacOsLibproc => "macos_libproc",
            Source::MacOsIokit => "macos_iokit",
            Source::Metal => "metal",
            Source::ServiceManager => "service_manager",
            Source::Derived => "derived",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Почему значение недоступно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// Платформа или драйвер не даёт такого счётчика.
    NotSupported,
    /// Нужны права администратора / root.
    RequiresElevation,
    /// Процесс защищён (PPL, системный).
    ProcessProtected,
    /// Устройство не найдено.
    NoSuchDevice,
    /// Счётчик есть, но драйвер его не отдаёт.
    CounterNotExposed,
    /// Значение ещё не измерено (первый снимок).
    NotMeasured,
    /// Данные устарели: источник не отвечает.
    Stale,
    /// Прочие ошибки чтения.
    ReadFailed,
}

impl UnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            UnavailableReason::NotSupported => "not_supported",
            UnavailableReason::RequiresElevation => "requires_elevation",
            UnavailableReason::ProcessProtected => "process_protected",
            UnavailableReason::NoSuchDevice => "no_such_device",
            UnavailableReason::CounterNotExposed => "counter_not_exposed",
            UnavailableReason::NotMeasured => "not_measured",
            UnavailableReason::Stale => "stale",
            UnavailableReason::ReadFailed => "read_failed",
        }
    }
}

impl fmt::Display for UnavailableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Значение с provenance.
///
/// `value == None` означает «данных нет». Это принципиально отличается от
/// `Some(0)`: ноль — это измеренный ноль, `None` — неизвестность.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counter<T> {
    /// Само значение; `None` — значение недоступно.
    pub value: Option<T>,
    /// Откуда получено значение.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Насколько можно доверять значению.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// Почему значения нет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<UnavailableReason>,
}

impl<T> Counter<T> {
    /// Значение измерено системным источником.
    pub fn measured(value: T, source: Source) -> Self {
        Self {
            value: Some(value),
            source: Some(source),
            confidence: Some(Confidence::Measured),
            unavailable_reason: None,
        }
    }

    /// Значение выведено из других данных.
    pub fn inferred(value: T, source: Source) -> Self {
        Self {
            value: Some(value),
            source: Some(source),
            confidence: Some(Confidence::Inferred),
            unavailable_reason: None,
        }
    }

    /// Значение оценено по косвенным признакам.
    pub fn estimated(value: T, source: Source) -> Self {
        Self {
            value: Some(value),
            source: Some(source),
            confidence: Some(Confidence::Estimated),
            unavailable_reason: None,
        }
    }

    /// Значение недоступно, и известно почему.
    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self {
            value: None,
            source: None,
            confidence: None,
            unavailable_reason: Some(reason),
        }
    }

    /// Значение доступно.
    pub fn is_available(&self) -> bool {
        self.value.is_some()
    }

    /// Значение недоступно.
    pub fn is_unavailable(&self) -> bool {
        self.value.is_none()
    }
}

impl<T: Copy> Counter<T> {
    /// Преобразует значение, сохраняя provenance.
    pub fn map<U: Copy>(self, f: impl FnOnce(T) -> U) -> Counter<U> {
        Counter {
            value: self.value.map(f),
            source: self.source,
            confidence: self.confidence,
            unavailable_reason: self.unavailable_reason,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Counter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Some(value) => write!(f, "{value}"),
            None => match self.unavailable_reason {
                Some(reason) => write!(f, "— ({reason})"),
                None => f.write_str("—"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_not_zero() {
        let counter: Counter<u64> = Counter::unavailable(UnavailableReason::RequiresElevation);
        assert!(counter.is_unavailable());
        assert!(!counter.is_available());
        assert_eq!(counter.value, None);
    }

    #[test]
    fn measured_zero_is_a_value() {
        let counter = Counter::measured(0u64, Source::LinuxProcFs);
        assert!(counter.is_available());
        assert_eq!(counter.value, Some(0));
        assert_eq!(counter.confidence, Some(Confidence::Measured));
    }

    #[test]
    fn map_keeps_provenance() {
        let counter = Counter::estimated(1024u64, Source::Derived).map(|v| v as f64);
        assert_eq!(counter.value, Some(1024.0));
        assert_eq!(counter.confidence, Some(Confidence::Estimated));
        assert_eq!(counter.source, Some(Source::Derived));
    }

    #[test]
    fn serializes_null_not_zero() {
        let counter: Counter<u64> = Counter::unavailable(UnavailableReason::NotSupported);
        let json = serde_json::to_string(&counter).unwrap();
        assert!(json.contains("\"value\":null"), "{json}");
        assert!(!json.contains(":0"), "{json}");
    }

    #[test]
    fn display_shows_reason() {
        let counter: Counter<u64> = Counter::unavailable(UnavailableReason::Stale);
        assert_eq!(counter.to_string(), "— (stale)");
    }
}
