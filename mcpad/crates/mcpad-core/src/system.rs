//! Системная сводка: CPU, память, ОС.

use serde::{Deserialize, Serialize};

use crate::counter::Counter;

/// Сведения о системе в одном снимке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Имя узла.
    pub hostname: Option<String>,
    /// Человекочитаемое имя ОС.
    pub os_name: String,
    /// Версия ОС.
    pub os_version: String,
    /// Архитектура (`x86_64`, `aarch64`).
    pub arch: String,
    /// Число логических процессоров.
    pub logical_cpus: u32,
    /// Время загрузки, секунды Unix.
    pub boot_time_unix: Option<u64>,
    /// Загрузка CPU в процентах от всей машины.
    pub cpu_percent: Counter<f64>,
    /// Накопительное занятое время CPU в наносекундах — основа для расчёта процента.
    pub cpu_time_total: Counter<u64>,
    /// Всего физической памяти, байты.
    pub memory_total: Counter<u64>,
    /// Доступно физической памяти, байты.
    pub memory_available: Counter<u64>,
    /// Размер файла подкачки, байты.
    pub pagefile_total: Counter<u64>,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            hostname: None,
            os_name: "unknown".to_string(),
            os_version: "unknown".to_string(),
            arch: "unknown".to_string(),
            logical_cpus: 0,
            boot_time_unix: None,
            cpu_percent: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            cpu_time_total: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            memory_total: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            memory_available: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            pagefile_total: Counter::unavailable(crate::UnavailableReason::NotMeasured),
        }
    }
}
