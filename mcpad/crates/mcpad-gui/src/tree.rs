//! Дерево процессов: раскрытие в плоский список строк.
//!
//! Дерево рисуется вручную, как и таблица: колонки должны стоять на своих
//! местах, а не «прижиматься» к правому краю каждой строкой отдельно. Поэтому
//! сначала дерево превращается в список строк, а потом рисуется одним проходом.

use std::collections::{HashMap, HashSet};

use mcpad_core::{ProcessInfo, ProcessTree};

/// Строка дерева.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    /// Идентификатор процесса.
    pub pid: u32,
    /// Имя образа.
    pub name: String,
    /// Глубина в дереве: корни — ноль.
    pub depth: u32,
    /// Доля CPU, проценты; `None` — не измерено.
    pub cpu: Option<f64>,
    /// Рабочий набор, байты; `None` — не измерено.
    pub memory: Option<u64>,
    /// Есть ли у узла потомки.
    pub expandable: bool,
    /// Свёрнут ли узел.
    pub collapsed: bool,
}

/// Раскрывает дерево в плоский список строк.
///
/// Свёрнутые ветки в список не попадают: их потомки просто не показываются.
pub fn flatten(
    tree: &ProcessTree,
    processes: &[ProcessInfo],
    collapsed: &HashSet<u32>,
) -> Vec<TreeRow> {
    let known: HashMap<u32, &ProcessInfo> = processes.iter().map(|p| (p.key.pid, p)).collect();
    let mut rows = Vec::new();
    for root in &tree.roots {
        push_row(*root, tree, &known, collapsed, 0, &mut rows);
    }
    rows
}

/// Добавляет узел и его потомков в список строк.
fn push_row(
    pid: u32,
    tree: &ProcessTree,
    known: &HashMap<u32, &ProcessInfo>,
    collapsed: &HashSet<u32>,
    depth: u32,
    rows: &mut Vec<TreeRow>,
) {
    // Глубина ограничена: дерево из данных может быть очень глубоким.
    if depth > 64 {
        return;
    }

    let children = tree.children_of(pid);
    let process = known.get(&pid);
    rows.push(TreeRow {
        pid,
        name: process
            .map(|process| process.name.clone())
            .unwrap_or_else(|| format!("pid {pid}")),
        depth,
        cpu: process.and_then(|process| process.cpu_percent.value),
        memory: process.and_then(|process| process.memory_working_set.value),
        expandable: !children.is_empty(),
        collapsed: collapsed.contains(&pid),
    });

    if collapsed.contains(&pid) {
        return;
    }
    for child in children {
        push_row(*child, tree, known, collapsed, depth + 1, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey, Snapshot, Source, SystemInfo};

    fn process(pid: u32, parent: Option<u32>, name: &str) -> ProcessInfo {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), name);
        process.parent_pid = parent;
        process.cpu_percent = Counter::measured(pid as f64, Source::Derived);
        process.memory_working_set = Counter::measured(pid as u64 * 1024, Source::Derived);
        process
    }

    fn tree_of(processes: &[ProcessInfo]) -> ProcessTree {
        ProcessTree::build(&Snapshot::new(SystemInfo::default(), processes.to_vec()))
    }

    #[test]
    fn children_come_after_their_parent() {
        let processes = vec![
            process(1, None, "root.exe"),
            process(2, Some(1), "child.exe"),
            process(3, Some(2), "grandchild.exe"),
        ];
        let rows = flatten(&tree_of(&processes), &processes, &HashSet::new());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].pid, 1);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].pid, 2);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].pid, 3);
        assert_eq!(rows[2].depth, 2);
    }

    #[test]
    fn collapsed_branch_hides_its_children() {
        let processes = vec![
            process(1, None, "root.exe"),
            process(2, Some(1), "child.exe"),
            process(3, Some(2), "grandchild.exe"),
        ];
        let mut collapsed = HashSet::new();
        collapsed.insert(2);
        let rows = flatten(&tree_of(&processes), &processes, &collapsed);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].pid, 2);
        assert!(rows[1].collapsed);
        assert!(rows[1].expandable);
    }

    #[test]
    fn metrics_come_from_the_snapshot() {
        let processes = vec![process(7, None, "app.exe")];
        let rows = flatten(&tree_of(&processes), &processes, &HashSet::new());
        assert_eq!(rows[0].cpu, Some(7.0));
        assert_eq!(rows[0].memory, Some(7 * 1024));
        assert_eq!(rows[0].name, "app.exe");
        assert!(!rows[0].expandable);
    }

    #[test]
    fn unknown_process_gets_a_fallback_name() {
        // Родителя нет в списке — процесс становится корнем дерева.
        let tree = tree_of(&[process(99, Some(1234), "ghost.exe")]);
        let rows = flatten(&tree, &[], &HashSet::new());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "pid 99");
        assert_eq!(rows[0].cpu, None);
        assert_eq!(rows[0].memory, None);
    }
}
