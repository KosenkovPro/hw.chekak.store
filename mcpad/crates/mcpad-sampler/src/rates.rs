//! Расчёт долей CPU из дельт между соседними снимками.
//!
//! Правило одно для всех платформ: доля процесса — это его дельта времени CPU,
//! делённая на прошедшее wall-время и на число логических процессоров.
//! Так «один горячий поток на 32-ядерной машине» показывает ~3 %, как и в
//! штатном диспетчере Windows.

use std::time::Duration;

use mcpad_core::{Confidence, Counter, Snapshot, Source};

/// Пересчитывает доли CPU в `current` по данным `previous`.
///
/// Если времени между снимками не прошло или процесс новый, его доля
/// остаётся неизвестной (`Unavailable`), а не нулевой.
pub fn apply_cpu_percent(current: &mut Snapshot, previous: &Snapshot, wall: Duration) {
    let wall_nanos = wall.as_nanos() as f64;
    if wall_nanos <= 0.0 {
        return;
    }

    let cpus = previous.system.logical_cpus.max(1) as f64;
    let denominator = wall_nanos * cpus;

    // Системная загрузка: дельта занятого времени CPU.
    if let (Some(current_busy), Some(previous_busy)) = (
        current.system.cpu_time_total.value,
        previous.system.cpu_time_total.value,
    ) {
        let delta = current_busy.saturating_sub(previous_busy) as f64;
        let percent = (delta / denominator * 100.0).clamp(0.0, 100.0);
        current.system.cpu_percent = Counter::measured(percent, Source::Derived);
    }

    for process in &mut current.processes {
        let Some(previous_process) = previous
            .processes
            .iter()
            .find(|candidate| candidate.key.same_process(&process.key))
        else {
            // Новый процесс: его история ещё не набрана.
            process.cpu_percent = Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured);
            continue;
        };

        let (Some(current_cpu), Some(previous_cpu)) = (
            process.cpu_time_total.value,
            previous_process.cpu_time_total.value,
        ) else {
            process.cpu_percent = Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured);
            continue;
        };

        let delta = current_cpu.saturating_sub(previous_cpu) as f64;
        let percent = (delta / denominator * 100.0).clamp(0.0, 100.0);
        process.cpu_percent = Counter::measured(percent, Source::Derived);
    }
}

/// Доля в процентах из дельт — отдельно, чтобы правило можно было проверить.
pub fn cpu_percent(delta_nanos: u64, wall: Duration, logical_cpus: u32) -> f64 {
    let denominator = wall.as_nanos() as f64 * logical_cpus.max(1) as f64;
    if denominator <= 0.0 {
        return 0.0;
    }
    (delta_nanos as f64 / denominator * 100.0).clamp(0.0, 100.0)
}

/// Проверка: значение с полем уверенности должно быть измеренным.
pub fn is_measured(counter: &Counter<f64>) -> bool {
    counter.confidence == Some(Confidence::Measured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{ProcessInfo, ProcessKey, SystemInfo};

    fn snapshot_with_cpu(pid: u32, cpu_nanos: u64, system_cpu_nanos: u64, cpus: u32) -> Snapshot {
        let mut process = ProcessInfo::new(ProcessKey::new(pid, Some(1), Some(1)), "proc");
        process.cpu_time_total = Counter::measured(cpu_nanos, Source::LinuxProcFs);

        let system = SystemInfo {
            logical_cpus: cpus,
            cpu_time_total: Counter::measured(system_cpu_nanos, Source::LinuxProcFs),
            ..SystemInfo::default()
        };

        Snapshot::new(system, vec![process])
    }

    #[test]
    fn one_busy_thread_on_32_cpus_is_about_three_percent() {
        let value = cpu_percent(1_000_000_000, Duration::from_secs(1), 32);
        assert!((value - 3.125).abs() < 0.01, "получилось {value}");
    }

    #[test]
    fn percent_is_clamped_to_hundred() {
        let value = cpu_percent(10_000_000_000, Duration::from_secs(1), 1);
        assert_eq!(value, 100.0);
    }

    #[test]
    fn applies_percent_to_matching_process() {
        let previous = snapshot_with_cpu(10, 0, 0, 4);
        let mut current = snapshot_with_cpu(10, 2_000_000_000, 2_000_000_000, 4);

        apply_cpu_percent(&mut current, &previous, Duration::from_secs(1));

        let percent = current.processes[0]
            .cpu_percent
            .value
            .expect("доля посчитана");
        assert!((percent - 50.0).abs() < 0.01, "получилось {percent}");
        assert!(is_measured(&current.processes[0].cpu_percent));
        assert!(current.system.cpu_percent.value.is_some());
    }

    #[test]
    fn new_process_stays_unknown() {
        let previous = snapshot_with_cpu(10, 0, 0, 4);
        let mut current = snapshot_with_cpu(11, 5_000_000_000, 0, 4);

        apply_cpu_percent(&mut current, &previous, Duration::from_secs(1));

        assert!(current.processes[0].cpu_percent.is_unavailable());
    }

    #[test]
    fn reused_pid_is_not_matched() {
        let previous = snapshot_with_cpu(10, 0, 0, 4);
        // Тот же PID, но другое время старта — другой процесс.
        let mut current = snapshot_with_cpu(10, 5_000_000_000, 0, 4);
        current.processes[0].key = ProcessKey::new(10, Some(999), Some(1));

        apply_cpu_percent(&mut current, &previous, Duration::from_secs(1));

        assert!(current.processes[0].cpu_percent.is_unavailable());
    }
}
