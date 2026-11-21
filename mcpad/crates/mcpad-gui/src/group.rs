//! Группировка процессов.
//!
//! Диспетчер задач Windows 11 и System Informer умеют сводить процессы в
//! группы: по приложению, по родителю, по пользователю. Пользователю это
//! важно — десятки процессов одного браузера иначе не отличить от одного
//! тяжёлого.
//!
//! Группировка считается по снимку, а не по дереву: у процесса может быть
//! пропавший родитель, и тогда «приложение» — это он сам.

use std::collections::HashMap;

use mcpad_core::ProcessInfo;

/// Как группировать процессы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    /// Без группировки: плоский список.
    None,
    /// По приложению: по корневому предку.
    Application,
    /// По непосредственному родителю.
    Parent,
}

impl GroupBy {
    /// Все варианты группировки.
    pub fn all() -> [GroupBy; 3] {
        [GroupBy::None, GroupBy::Application, GroupBy::Parent]
    }

    /// Группировка по коду из командной строки.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "none" | "flat" | "без" => Some(GroupBy::None),
            "application" | "app" | "apps" | "приложение" | "приложению" => {
                Some(GroupBy::Application)
            }
            "parent" | "родитель" | "родителю" => Some(GroupBy::Parent),
            _ => None,
        }
    }

    /// Код группировки для командной строки.
    pub fn code(self) -> &'static str {
        match self {
            GroupBy::None => "none",
            GroupBy::Application => "application",
            GroupBy::Parent => "parent",
        }
    }
}

/// Группа процессов.
#[derive(Debug, Clone)]
pub struct Group<'a> {
    /// Заголовок группы.
    pub label: String,
    /// Процессы группы.
    pub rows: Vec<&'a ProcessInfo>,
}

impl<'a> Group<'a> {
    /// Суммарная доля CPU группы; `None` — если у части процессов значение
    /// неизвестно: тогда и сумма неизвестна.
    pub fn cpu(&self) -> Option<f64> {
        let mut sum = 0.0;
        for process in &self.rows {
            sum += process.cpu_percent.value?;
        }
        Some(sum)
    }

    /// Суммарный рабочий набор группы, байты; `None` — если у части процессов
    /// значение неизвестно.
    pub fn memory(&self) -> Option<u64> {
        let mut sum = 0u64;
        for process in &self.rows {
            sum = sum.saturating_add(process.memory_working_set.value?);
        }
        Some(sum)
    }

    /// Число процессов с неизвестным значением CPU.
    pub fn unknown_cpu(&self) -> usize {
        self.rows
            .iter()
            .filter(|process| process.cpu_percent.value.is_none())
            .count()
    }

    /// Число процессов с неизвестным рабочим набором.
    pub fn unknown_memory(&self) -> usize {
        self.rows
            .iter()
            .filter(|process| process.memory_working_set.value.is_none())
            .count()
    }
}

/// Группирует процессы по выбранному правилу.
///
/// Порядок групп: по убыванию суммарного CPU, группы без измеренного CPU —
/// в конце, внутри одинаковых — по названию. Внутри группы порядок задаёт
/// вызывающий код: он сортирует строки сам.
pub fn group<'a>(rows: &[&'a ProcessInfo], by: GroupBy) -> Vec<Group<'a>> {
    if by == GroupBy::None {
        return vec![Group {
            label: String::new(),
            rows: rows.to_vec(),
        }];
    }

    let known: HashMap<u32, &ProcessInfo> = rows
        .iter()
        .map(|process| (process.key.pid, *process))
        .collect();

    let mut groups: Vec<Group<'a>> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for process in rows {
        let label = match by {
            GroupBy::None => String::new(),
            GroupBy::Application => {
                let root = root_ancestor(&known, process.key.pid);
                known
                    .get(&root)
                    .map(|root| root.name.clone())
                    .unwrap_or_else(|| format!("pid {root}"))
            }
            GroupBy::Parent => match process.parent_pid {
                Some(parent) => known
                    .get(&parent)
                    .map(|parent| parent.name.clone())
                    .unwrap_or_else(|| format!("pid {parent}")),
                None => process.name.clone(),
            },
        };

        match index.get(&label) {
            Some(position) => groups[*position].rows.push(process),
            None => {
                index.insert(label.clone(), groups.len());
                groups.push(Group {
                    label,
                    rows: vec![process],
                });
            }
        }
    }

    groups.sort_by(|left, right| {
        let ordering = match (left.cpu(), right.cpu()) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left_cpu), Some(right_cpu)) => right_cpu.total_cmp(&left_cpu),
        };
        ordering.then_with(|| left.label.cmp(&right.label))
    });

    groups
}

/// Корневой предок процесса: верхний живой родитель.
fn root_ancestor(known: &HashMap<u32, &ProcessInfo>, pid: u32) -> u32 {
    let mut current = pid;
    // Глубина ограничена: в данных возможны циклы родителей.
    for _ in 0..64 {
        let Some(parent) = known.get(&current).and_then(|process| process.parent_pid) else {
            break;
        };
        if !known.contains_key(&parent) {
            break;
        }
        current = parent;
    }
    current
}

/// Системный ли процесс.
///
/// Системными считаются процессы, корневой предок которых — сама система:
/// на Windows это PID 4 («System»), на Linux — init (PID 1), на macOS —
/// launchd (PID 1). Такой фильтр есть в Task Explorer («Hide system
/// processes»).
pub fn is_system_process(process: &ProcessInfo, known: &HashMap<u32, &ProcessInfo>) -> bool {
    let root = root_ancestor(known, process.key.pid);
    root == 1 || root == 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey, Source};

    fn process(pid: u32, name: &str, parent: Option<u32>, cpu: Option<f64>) -> ProcessInfo {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), name);
        process.parent_pid = parent;
        if let Some(value) = cpu {
            process.cpu_percent = Counter::measured(value, Source::Derived);
        }
        process.memory_working_set = Counter::measured(1024, Source::Derived);
        process
    }

    fn rows(processes: &[ProcessInfo]) -> Vec<&ProcessInfo> {
        processes.iter().collect()
    }

    #[test]
    fn no_grouping_keeps_one_group() {
        let processes = vec![
            process(1, "a.exe", None, Some(1.0)),
            process(2, "b.exe", None, None),
        ];
        let groups = group(&rows(&processes), GroupBy::None);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].rows.len(), 2);
        assert!(groups[0].label.is_empty());
    }

    #[test]
    fn application_grouping_uses_root_ancestor() {
        let processes = vec![
            process(1, "browser.exe", None, Some(5.0)),
            process(2, "tab.exe", Some(1), Some(3.0)),
            process(3, "gpu.exe", Some(2), Some(2.0)),
            process(4, "other.exe", None, Some(1.0)),
        ];
        let groups = group(&rows(&processes), GroupBy::Application);
        assert_eq!(groups.len(), 2);

        let browser = groups
            .iter()
            .find(|group| group.label == "browser.exe")
            .expect("группа браузера есть");
        assert_eq!(browser.rows.len(), 3);
        assert_eq!(browser.cpu(), Some(10.0));

        let other = groups
            .iter()
            .find(|group| group.label == "other.exe")
            .expect("вторая группа есть");
        assert_eq!(other.rows.len(), 1);
    }

    #[test]
    fn parent_grouping_uses_direct_parent() {
        let processes = vec![
            process(1, "browser.exe", None, Some(5.0)),
            process(2, "tab.exe", Some(1), Some(3.0)),
            process(3, "worker.exe", Some(2), Some(1.0)),
        ];
        let groups = group(&rows(&processes), GroupBy::Parent);
        // Группа «browser.exe»: сам браузер (родителя нет — группа по своему
        // имени) и его непосредственный потомок. «worker.exe» — уже внук,
        // поэтому он в группе своего родителя «tab.exe».
        assert_eq!(groups.len(), 2);

        let browser = groups
            .iter()
            .find(|group| group.label == "browser.exe")
            .expect("группа родителя есть");
        assert_eq!(browser.rows.len(), 2);
        assert_eq!(browser.cpu(), Some(8.0));

        let tab = groups
            .iter()
            .find(|group| group.label == "tab.exe")
            .expect("группа дедушки есть");
        assert_eq!(tab.rows.len(), 1);
        assert_eq!(tab.rows[0].key.pid, 3);
    }

    #[test]
    fn group_with_unknown_cpu_has_no_total() {
        // Один процесс с общим предком: группа собирает оба значения.
        let processes = vec![
            process(1, "a.exe", None, Some(1.0)),
            process(2, "b.exe", Some(1), None),
        ];
        let groups = group(&rows(&processes), GroupBy::Application);
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        // Сумма неизвестна: часть значений не измерена.
        assert_eq!(group.cpu(), None);
        assert_eq!(group.unknown_cpu(), 1);
        assert_eq!(group.memory(), Some(2048));
    }

    #[test]
    fn groups_without_cpu_go_last() {
        let processes = vec![
            process(1, "measured.exe", None, Some(1.0)),
            process(2, "unknown.exe", None, None),
        ];
        let groups = group(&rows(&processes), GroupBy::Application);
        assert_eq!(groups[0].label, "measured.exe");
        assert_eq!(groups[1].label, "unknown.exe");
    }

    #[test]
    fn parent_cycle_does_not_hang() {
        let mut first = process(1, "a.exe", Some(2), Some(1.0));
        let mut second = process(2, "b.exe", Some(1), Some(1.0));
        first.parent_pid = Some(2);
        second.parent_pid = Some(1);
        let processes = vec![first, second];
        let groups = group(&rows(&processes), GroupBy::Application);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn group_by_code_parses_command_line_value() {
        assert_eq!(GroupBy::from_code("none"), Some(GroupBy::None));
        assert_eq!(
            GroupBy::from_code("application"),
            Some(GroupBy::Application)
        );
        assert_eq!(GroupBy::from_code("PARENT"), Some(GroupBy::Parent));
        assert_eq!(GroupBy::from_code("приложению"), Some(GroupBy::Application));
        assert_eq!(GroupBy::from_code("непонятно"), None);
    }

    #[test]
    fn system_processes_are_recognised() {
        let processes = [
            process(4, "System", None, Some(1.0)),
            process(100, "services.exe", Some(4), Some(1.0)),
            process(200, "app.exe", Some(100), Some(1.0)),
        ];
        let known: HashMap<u32, &ProcessInfo> = processes
            .iter()
            .map(|process| (process.key.pid, process))
            .collect();

        assert!(is_system_process(&processes[0], &known));
        assert!(is_system_process(&processes[1], &known));
        // Пользовательское приложение под системным деревом — тоже системное:
        // корневой предок у него тот же.
        assert!(is_system_process(&processes[2], &known));

        let user = [process(500, "app.exe", None, Some(1.0))];
        let known: HashMap<u32, &ProcessInfo> = user.iter().map(|p| (p.key.pid, p)).collect();
        assert!(!is_system_process(&user[0], &known));
    }
}
