//! Текстовое представление снимка.
//!
//! Недоступные значения печатаются как «—», а не как ноль: ноль — это
//! измеренный ноль, «—» — «данных нет».

use mcpad_core::{Counter, Snapshot};

/// Сортировка списка процессов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Sort {
    /// По доле CPU (по убыванию).
    Cpu,
    /// По памяти (по убыванию).
    Memory,
    /// По имени.
    Name,
    /// По PID.
    Pid,
}

/// Форматирует значение счётчика для таблицы.
///
/// Неизвестное значение — прочерк, а не ноль.
pub fn format_counter(counter: &Counter<f64>, width: usize) -> String {
    let text = match counter.value {
        Some(value) => format!("{value:.1}"),
        None => "—".to_string(),
    };
    format!("{text:>width$}")
}

/// Форматирует байты в мегабайты.
pub fn format_mib(counter: &Counter<u64>) -> String {
    match counter.value {
        Some(bytes) => format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)),
        None => "—".to_string(),
    }
}

/// Форматирует целое число счётчика.
pub fn format_count(counter: &Counter<u32>) -> String {
    match counter.value {
        Some(value) => value.to_string(),
        None => "—".to_string(),
    }
}

/// Печатает таблицу процессов.
pub fn render_processes(snapshot: &Snapshot, sort: Sort, limit: usize) -> String {
    let mut processes: Vec<&mcpad_core::ProcessInfo> = snapshot.processes.iter().collect();

    match sort {
        Sort::Cpu => processes.sort_by(|a, b| {
            b.cpu_percent
                .value
                .unwrap_or(f64::MIN)
                .total_cmp(&a.cpu_percent.value.unwrap_or(f64::MIN))
        }),
        Sort::Memory => processes.sort_by(|a, b| {
            b.memory_working_set
                .value
                .unwrap_or(0)
                .cmp(&a.memory_working_set.value.unwrap_or(0))
        }),
        Sort::Name => processes.sort_by(|a, b| a.name.cmp(&b.name)),
        Sort::Pid => processes.sort_by_key(|process| process.key.pid),
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{:>7}  {:<28}  {:>6}  {:>9}  {:>5}  {:>7}\n",
        "PID", "NAME", "CPU%", "MEM(MiB)", "THR", "HANDLES"
    ));
    out.push_str(&"-".repeat(72));
    out.push('\n');

    for process in processes.iter().take(limit) {
        out.push_str(&format!(
            "{:>7}  {:<28}  {}  {:>9}  {:>5}  {:>7}\n",
            process.key.pid,
            truncate(&process.name, 28),
            format_counter(&process.cpu_percent, 6),
            format_mib(&process.memory_working_set),
            format_count(&process.thread_count),
            format_count(&process.handle_count),
        ));
    }

    out.push_str(&format!(
        "\nВсего процессов: {}\n",
        snapshot.processes.len()
    ));

    out
}

/// Печатает сводку по системе.
pub fn render_system(snapshot: &Snapshot) -> String {
    let system = &snapshot.system;
    let mut out = String::new();
    out.push_str(&format!(
        "Система: {} {} ({}), ядер: {}\n",
        system.os_name, system.os_version, system.arch, system.logical_cpus
    ));
    if let Some(hostname) = &system.hostname {
        out.push_str(&format!("Узел: {hostname}\n"));
    }
    out.push_str(&format!(
        "CPU: {}%  память: {} из {} MiB\n",
        format_counter(&system.cpu_percent, 6),
        format_mib(&system.memory_available),
        format_mib(&system.memory_total)
    ));
    out
}

/// Обрезает строку до нужной длины.
fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut result: String = text.chars().take(width.saturating_sub(1)).collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{ProcessInfo, ProcessKey, SystemInfo};

    fn snapshot() -> Snapshot {
        let mut first = ProcessInfo::new(ProcessKey::from_pid(1), "system");
        first.cpu_percent = mcpad_core::Counter::measured(3.5, mcpad_core::Source::Derived);
        first.memory_working_set =
            mcpad_core::Counter::measured(1024 * 1024, mcpad_core::Source::Derived);
        first.thread_count = mcpad_core::Counter::measured(8, mcpad_core::Source::Derived);

        let mut second = ProcessInfo::new(ProcessKey::from_pid(2), "app");
        second.cpu_percent =
            mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured);
        second.memory_working_set =
            mcpad_core::Counter::unavailable(mcpad_core::UnavailableReason::RequiresElevation);

        Snapshot::new(SystemInfo::default(), vec![first, second])
    }

    #[test]
    fn unknown_values_are_dashes() {
        let text = render_processes(&snapshot(), Sort::Cpu, 10);
        assert!(
            text.contains('—'),
            "недоступное значение должно быть тире:\n{text}"
        );
        assert!(
            !text.contains("0.0"),
            "неизвестное значение не должно печататься нулём:\n{text}"
        );
    }

    #[test]
    fn sorting_by_cpu_puts_measured_first() {
        let text = render_processes(&snapshot(), Sort::Cpu, 10);
        let system_line = text.find("system").expect("строка system");
        let app_line = text.find("app").expect("строка app");
        assert!(system_line < app_line);
    }

    #[test]
    fn limit_is_respected() {
        let text = render_processes(&snapshot(), Sort::Pid, 1);
        assert_eq!(text.matches("  1  ").count(), 1);
    }

    #[test]
    fn long_names_are_truncated() {
        let long = "x".repeat(50);
        assert_eq!(truncate(&long, 10).chars().count(), 10);
        assert_eq!(truncate("short", 10), "short");
    }
}
