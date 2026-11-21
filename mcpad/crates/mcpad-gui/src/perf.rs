//! История измерений для раздела «Производительность».
//!
//! Графику нужно не «последнее значение», а ряд во времени. Здесь живут
//! кольцевые буферы: по одному на системные величины и по одному на
//! отслеживаемый процесс.
//!
//! Главное правило — то же, что и в ядре: **пропуск измерения остаётся
//! пропуском**. Если счётчик был недоступен, в буфер кладётся `None`, и на
//! графике это разрыв, а не линия на нуле.

use std::collections::HashMap;

use mcpad_core::Snapshot;
use mcpad_series::{Series, TimeSpan, Timeline};

/// Сколько отсчётов держим в буфере.
const CAPACITY: usize = 600;

/// Сколько процессов отслеживаем по отдельности.
const TRACKED_PROCESSES: usize = 20;

/// Сколько рядов по процессам держим одновременно.
const MAX_SERIES: usize = 64;

/// История измерений.
#[derive(Debug, Clone)]
pub struct PerfHistory {
    /// Загрузка CPU системы, проценты.
    pub system_cpu: Series,
    /// Занятая память, проценты.
    pub memory_used: Series,
    /// Доля CPU по процессам, проценты.
    pub processes: HashMap<u32, Series>,
    /// Окно времени и курсор.
    pub timeline: Timeline,
    /// Момент последнего снимка, миллисекунды Unix.
    pub last_at_unix_ms: u64,
}

impl PerfHistory {
    pub fn new() -> Self {
        Self {
            system_cpu: Series::with_capacity(CAPACITY),
            memory_used: Series::with_capacity(CAPACITY),
            processes: HashMap::new(),
            timeline: Timeline::new(TimeSpan::OneMinute),
            last_at_unix_ms: 0,
        }
    }

    /// Добавляет снимок в историю.
    pub fn push_snapshot(&mut self, snapshot: &Snapshot) {
        let at = snapshot.taken_at_unix_ms;
        self.last_at_unix_ms = at;

        match snapshot.system.cpu_percent.value {
            Some(value) => self.system_cpu.push_value(at, value),
            None => self.system_cpu.push_gap(at),
        }

        match memory_used_percent(snapshot) {
            Some(value) => self.memory_used.push_value(at, value),
            None => self.memory_used.push_gap(at),
        }

        // Известные значения CPU по процессам.
        let mut known: HashMap<u32, f64> = HashMap::new();
        let mut ranked: Vec<(u32, f64)> = Vec::new();
        for process in &snapshot.processes {
            if let Some(value) = process.cpu_percent.value {
                known.insert(process.key.pid, value);
                ranked.push((process.key.pid, value));
            }
        }
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

        // Под наблюдение берём самых нагруженных.
        for (pid, _) in ranked.iter().take(TRACKED_PROCESSES) {
            self.processes
                .entry(*pid)
                .or_insert_with(|| Series::with_capacity(CAPACITY));
        }

        // Обновляем уже отслеживаемых: значение есть — пишем его, нет — пропуск.
        let tracked: Vec<u32> = self.processes.keys().copied().collect();
        for pid in tracked {
            let value = known.get(&pid).copied();
            if let Some(series) = self.processes.get_mut(&pid) {
                match value {
                    Some(value) => series.push_value(at, value),
                    None => series.push_gap(at),
                }
            }
        }

        self.prune();
    }

    /// Ряд процесса по PID.
    pub fn process_series(&self, pid: u32) -> Option<&Series> {
        self.processes.get(&pid)
    }

    /// Процессы с последним известным значением CPU, от большего к меньшему.
    pub fn top_processes(&self, limit: usize) -> Vec<(u32, f64)> {
        let mut values: Vec<(u32, f64)> = self
            .processes
            .iter()
            .filter_map(|(pid, series)| series.last_value().map(|value| (*pid, value)))
            .collect();
        values.sort_by(|a, b| b.1.total_cmp(&a.1));
        values.truncate(limit);
        values
    }

    /// Убирает самые старые ряды, если процессов слишком много.
    fn prune(&mut self) {
        while self.processes.len() > MAX_SERIES {
            let oldest = self
                .processes
                .iter()
                .min_by_key(|(_, series)| series.len())
                .map(|(pid, _)| *pid);
            match oldest {
                Some(pid) => {
                    self.processes.remove(&pid);
                }
                None => break,
            }
        }
    }
}

impl Default for PerfHistory {
    fn default() -> Self {
        PerfHistory::new()
    }
}

/// Занятая память в процентах.
///
/// Если хоть одно из значений неизвестно, результат неизвестен: считать
/// «занято 0%» по неполным данным нельзя.
fn memory_used_percent(snapshot: &Snapshot) -> Option<f64> {
    let total = snapshot.system.memory_total.value? as f64;
    let available = snapshot.system.memory_available.value? as f64;
    if total <= 0.0 {
        return None;
    }
    Some(((total - available) / total * 100.0).clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessInfo, ProcessKey, Source, SystemInfo};

    fn snapshot_with(cpu: Option<f64>, memory: Option<(u64, u64)>) -> Snapshot {
        let mut system = SystemInfo {
            cpu_percent: match cpu {
                Some(value) => Counter::measured(value, Source::Derived),
                None => Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured),
            },
            ..SystemInfo::default()
        };
        if let Some((total, available)) = memory {
            system.memory_total = Counter::measured(total, Source::LinuxProcFs);
            system.memory_available = Counter::measured(available, Source::LinuxProcFs);
        }

        let processes: Vec<ProcessInfo> = (1..=3)
            .map(|pid| {
                let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), format!("p{pid}"));
                if let Some(value) = cpu {
                    process.cpu_percent = Counter::measured(value, Source::Derived);
                }
                process
            })
            .collect();

        Snapshot::new(system, processes)
    }

    #[test]
    fn unknown_cpu_becomes_gap() {
        let mut history = PerfHistory::new();
        history.push_snapshot(&snapshot_with(None, None));

        // Измеренных значений нет — сводки тоже нет.
        assert!(history.system_cpu.stats(0, u64::MAX).is_none());

        // Но в ряду стоит именно пропуск, а не ноль.
        let samples: Vec<&mcpad_series::Sample> = history.system_cpu.samples().collect();
        assert_eq!(samples.len(), 1);
        assert!(samples[0].value.is_none());
    }

    #[test]
    fn measured_cpu_is_stored() {
        let mut history = PerfHistory::new();
        history.push_snapshot(&snapshot_with(Some(42.0), None));
        assert_eq!(history.system_cpu.last_value(), Some(42.0));
    }

    #[test]
    fn memory_percent_is_computed() {
        let mut history = PerfHistory::new();
        history.push_snapshot(&snapshot_with(Some(10.0), Some((1000, 250))));
        assert_eq!(history.memory_used.last_value(), Some(75.0));
    }

    #[test]
    fn unknown_memory_becomes_gap() {
        let mut history = PerfHistory::new();
        history.push_snapshot(&snapshot_with(Some(10.0), None));

        assert!(history.memory_used.stats(0, u64::MAX).is_none());
        let samples: Vec<&mcpad_series::Sample> = history.memory_used.samples().collect();
        assert_eq!(samples.len(), 1);
        assert!(samples[0].value.is_none());
    }

    #[test]
    fn process_series_are_tracked() {
        let mut history = PerfHistory::new();
        history.push_snapshot(&snapshot_with(Some(10.0), None));
        assert!(history.process_series(1).is_some());
        assert_eq!(history.process_series(1).unwrap().last_value(), Some(10.0));
    }

    #[test]
    fn tracked_process_losing_cpu_becomes_gap() {
        let mut history = PerfHistory::new();
        // Первый снимок: CPU известен, процесс попадает под наблюдение.
        history.push_snapshot(&snapshot_with(Some(10.0), None));
        // Второй снимок: CPU неизвестен — это пропуск, а не ноль.
        history.push_snapshot(&snapshot_with(None, None));

        let series = history.process_series(1).expect("ряд есть");
        let stats = series.stats(0, u64::MAX).expect("сводка есть");
        assert_eq!(stats.measured, 1);
        assert_eq!(stats.gaps, 1);
        assert_eq!(stats.avg, 10.0);
    }

    #[test]
    fn top_processes_are_sorted() {
        let mut history = PerfHistory::new();
        let mut snapshot = snapshot_with(None, None);
        for (index, process) in snapshot.processes.iter_mut().enumerate() {
            process.cpu_percent = Counter::measured((index + 1) as f64 * 10.0, Source::Derived);
        }
        history.push_snapshot(&snapshot);
        let top = history.top_processes(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].1 > top[1].1);
    }

    #[test]
    fn series_are_pruned() {
        let mut history = PerfHistory::new();
        let system = SystemInfo {
            cpu_percent: Counter::measured(1.0, Source::Derived),
            ..SystemInfo::default()
        };
        let processes: Vec<ProcessInfo> = (1..=200)
            .map(|pid| {
                let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), format!("p{pid}"));
                process.cpu_percent = Counter::measured(pid as f64, Source::Derived);
                process
            })
            .collect();
        history.push_snapshot(&Snapshot::new(system, processes));
        // Отслеживаются только самые нагруженные, буфер остаётся небольшим.
        assert!(history.processes.len() <= TRACKED_PROCESSES);
    }
}
