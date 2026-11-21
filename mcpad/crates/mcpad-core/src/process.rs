//! Процессы: устойчивая идентичность и сводка по одному процессу.

use serde::{Deserialize, Serialize};

use crate::counter::Counter;

/// Устойчивая идентичность процесса.
///
/// PID переиспользуется, поэтому одного PID недостаточно: ключ включает время
/// старта и идентификатор загрузки системы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessKey {
    /// Идентификатор процесса в ОС.
    pub pid: u32,
    /// Время старта процесса в секундах Unix; `None`, если ОС его не отдаёт.
    pub start_time_unix: Option<u64>,
    /// Хеш идентификатора загрузки; отличает процессы до и после ребута.
    pub boot_id: Option<u64>,
}

impl ProcessKey {
    pub fn new(pid: u32, start_time_unix: Option<u64>, boot_id: Option<u64>) -> Self {
        Self {
            pid,
            start_time_unix,
            boot_id,
        }
    }

    /// Ключ только по PID — для случаев, когда время старта неизвестно.
    pub fn from_pid(pid: u32) -> Self {
        Self::new(pid, None, None)
    }

    /// Один и тот же ли это процесс.
    ///
    /// Если время старта известно у обоих, сравнивается оно; иначе сравниваются
    /// только PID и загрузка — с оговоркой, что переиспользование PID в таком
    /// случае неразличимо.
    pub fn same_process(&self, other: &Self) -> bool {
        self.pid == other.pid
            && self.boot_id == other.boot_id
            && match (self.start_time_unix, other.start_time_unix) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// Сводка по одному процессу.
///
/// Единицы измерения зафиксированы: **байты**, **наносекунды**, **проценты**,
/// **штуки**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Устойчивая идентичность.
    pub key: ProcessKey,
    /// Имя образа (имя процесса).
    pub name: String,
    /// Полный путь к исполняемому файлу.
    pub exe_path: Option<String>,
    /// Командная строка.
    pub command_line: Option<String>,
    /// PID родителя.
    pub parent_pid: Option<u32>,
    /// Владелец процесса.
    pub user: Option<String>,
    /// Накопительное время CPU в наносекундах — основа для расчёта процента.
    pub cpu_time_total: Counter<u64>,
    /// Доля CPU в процентах от всей машины (вычисляется сэмплером из дельт).
    pub cpu_percent: Counter<f64>,
    /// Working set в байтах.
    pub memory_working_set: Counter<u64>,
    /// Приватная память (commit) в байтах.
    pub memory_private: Counter<u64>,
    /// Число потоков.
    pub thread_count: Counter<u32>,
    /// Число дескрипторов.
    pub handle_count: Counter<u32>,
    /// Прочитано из файлов/устройств, байт.
    pub io_read_bytes: Counter<u64>,
    /// Записано в файлы/устройства, байт.
    pub io_write_bytes: Counter<u64>,
    /// Процесс защищён системой (PPL и подобное).
    ///
    /// `None` — «определить не удалось»: не «не защищён». Признак требует
    /// отдельного запроса к каждому процессу, поэтому в горячем пути остаётся
    /// неизвестным.
    pub is_protected: Option<bool>,
}

impl ProcessInfo {
    /// Пустая сводка с ключом — база для заполнения бэкендом.
    pub fn new(key: ProcessKey, name: impl Into<String>) -> Self {
        Self {
            key,
            name: name.into(),
            exe_path: None,
            command_line: None,
            parent_pid: None,
            user: None,
            cpu_time_total: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            cpu_percent: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            memory_working_set: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            memory_private: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            thread_count: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            handle_count: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            io_read_bytes: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            io_write_bytes: Counter::unavailable(crate::UnavailableReason::NotMeasured),
            is_protected: None,
        }
    }
}

/// Подробности процесса — собираются по запросу, а не в горячем пути.
///
/// Полный путь и командная строка требуют открытия процесса, поэтому
/// сэмплер их не читает: он встал бы на колено при тысяче процессов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessDetails {
    /// Устойчивая идентификация.
    pub key: ProcessKey,
    /// Полный путь к исполняемому файлу.
    pub exe_path: Option<String>,
    /// Командная строка.
    pub command_line: Option<String>,
    /// Владелец процесса.
    pub user: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_process_requires_same_start_time() {
        let a = ProcessKey::new(42, Some(100), Some(7));
        let b = ProcessKey::new(42, Some(101), Some(7));
        let c = ProcessKey::new(42, Some(100), Some(7));
        assert!(!a.same_process(&b));
        assert!(a.same_process(&c));
    }

    #[test]
    fn same_process_without_start_time_falls_back_to_pid() {
        let a = ProcessKey::from_pid(42);
        let b = ProcessKey::from_pid(42);
        assert!(a.same_process(&b));
    }

    #[test]
    fn different_boot_is_different_process() {
        let a = ProcessKey::new(42, Some(100), Some(1));
        let b = ProcessKey::new(42, Some(100), Some(2));
        assert!(!a.same_process(&b));
    }
}
