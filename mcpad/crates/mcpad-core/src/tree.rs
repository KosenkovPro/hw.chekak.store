//! Дерево процессов: из плоского списка — в структуру родителей и детей.
//!
//! Снимок — плоский список, как его отдаёт ОС. Дерево собирается отдельно,
//! чтобы и GUI, и CLI показывали одно и то же.
//!
//! Данные из ОС могут быть странными (родитель исчез, ссылка на себя), поэтому
//! построение защищено от циклов и от висячих ссылок.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::snapshot::Snapshot;

/// Дерево процессов.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessTree {
    /// Процессы без родителя (или с пропавшим родителем).
    pub roots: Vec<u32>,
    /// Дети каждого процесса.
    pub children: HashMap<u32, Vec<u32>>,
    /// Глубина каждого процесса.
    pub depth: HashMap<u32, u32>,
}

impl ProcessTree {
    /// Строит дерево по снимку.
    pub fn build(snapshot: &Snapshot) -> Self {
        let known: HashSet<u32> = snapshot.processes.iter().map(|p| p.key.pid).collect();
        let mut parents: HashMap<u32, u32> = HashMap::new();

        for process in &snapshot.processes {
            if let Some(parent) = process.parent_pid {
                // Родитель должен существовать и не быть самим процессом.
                if parent != process.key.pid && known.contains(&parent) {
                    parents.insert(process.key.pid, parent);
                }
            }
        }

        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (pid, parent) in &parents {
            children.entry(*parent).or_default().push(*pid);
        }
        for list in children.values_mut() {
            list.sort_unstable();
        }

        let roots: Vec<u32> = snapshot
            .processes
            .iter()
            .map(|process| process.key.pid)
            .filter(|pid| !parents.contains_key(pid))
            .collect();

        let mut tree = ProcessTree {
            roots,
            children,
            depth: HashMap::new(),
        };
        tree.compute_depth();
        tree
    }

    /// Считает глубину обходом в ширину: циклы не зацикливают обход.
    fn compute_depth(&mut self) {
        let mut depth: HashMap<u32, u32> = HashMap::new();
        let mut queue: VecDeque<u32> = VecDeque::new();

        for root in &self.roots {
            depth.insert(*root, 0);
            queue.push_back(*root);
        }

        while let Some(pid) = queue.pop_front() {
            let current = depth.get(&pid).copied().unwrap_or(0);
            for child in self.children.get(&pid).into_iter().flatten() {
                if depth.contains_key(child) {
                    continue;
                }
                depth.insert(*child, current + 1);
                queue.push_back(*child);
            }
        }

        // Процессы вне дерева (цикл) — глубина неизвестна.
        self.depth = depth;
    }

    /// Дети процесса.
    pub fn children_of(&self, pid: u32) -> &[u32] {
        self.children.get(&pid).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Глубина процесса; `None`, если процесс не попал в дерево.
    pub fn depth_of(&self, pid: u32) -> Option<u32> {
        self.depth.get(&pid).copied()
    }

    /// Является ли процесс корнем.
    pub fn is_root(&self, pid: u32) -> bool {
        self.roots.contains(&pid)
    }

    /// Число процессов в дереве.
    pub fn len(&self) -> usize {
        self.depth.len()
    }

    pub fn is_empty(&self) -> bool {
        self.depth.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessInfo, ProcessKey};
    use crate::system::SystemInfo;

    fn snapshot_with(relations: &[(u32, Option<u32>)]) -> Snapshot {
        let processes = relations
            .iter()
            .map(|(pid, parent)| {
                let mut process = ProcessInfo::new(ProcessKey::from_pid(*pid), format!("p{pid}"));
                process.parent_pid = *parent;
                process
            })
            .collect();
        Snapshot::new(SystemInfo::default(), processes)
    }

    #[test]
    fn builds_hierarchy() {
        let snapshot = snapshot_with(&[(1, None), (2, Some(1)), (3, Some(2))]);
        let tree = ProcessTree::build(&snapshot);

        assert_eq!(tree.roots, vec![1]);
        assert_eq!(tree.children_of(1), &[2]);
        assert_eq!(tree.children_of(2), &[3]);
        assert_eq!(tree.depth_of(1), Some(0));
        assert_eq!(tree.depth_of(3), Some(2));
    }

    #[test]
    fn missing_parent_becomes_root() {
        let snapshot = snapshot_with(&[(1, Some(99))]);
        let tree = ProcessTree::build(&snapshot);
        assert_eq!(tree.roots, vec![1]);
    }

    #[test]
    fn self_parent_becomes_root() {
        let snapshot = snapshot_with(&[(7, Some(7))]);
        let tree = ProcessTree::build(&snapshot);
        assert_eq!(tree.roots, vec![7]);
    }

    #[test]
    fn cycles_do_not_hang() {
        let snapshot = snapshot_with(&[(1, Some(2)), (2, Some(1))]);
        let tree = ProcessTree::build(&snapshot);
        // Ни один из цикла не является корнем, но обход не зацикливается.
        assert!(tree.roots.is_empty());
        assert!(tree.len() <= 2);
    }

    #[test]
    fn siblings_are_sorted() {
        let snapshot = snapshot_with(&[(1, None), (5, Some(1)), (3, Some(1))]);
        let tree = ProcessTree::build(&snapshot);
        assert_eq!(tree.children_of(1), &[3, 5]);
    }
}
