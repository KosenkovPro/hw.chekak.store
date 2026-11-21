//! Снимок системы — единица обмена данными между слоями MCPad.

use serde::{Deserialize, Serialize};

use crate::process::{ProcessInfo, ProcessKey};
use crate::system::SystemInfo;

/// Версия схемы снимка. Повышается при несовместимых изменениях полей.
pub const SCHEMA_VERSION: u32 = 1;

/// Снимок системы: сводка плюс список процессов.
///
/// Один и тот же снимок читают GUI, CLI, MCP и удалённый клиент, поэтому
/// формат зафиксирован JSON-схемой в `schema/snapshot.schema.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Версия схемы снимка.
    pub schema_version: u32,
    /// Время снятия снимка, миллисекунды Unix.
    pub taken_at_unix_ms: u64,
    /// Сводка по системе.
    pub system: SystemInfo,
    /// Процессы.
    pub processes: Vec<ProcessInfo>,
}

impl Snapshot {
    pub fn new(system: SystemInfo, processes: Vec<ProcessInfo>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            taken_at_unix_ms: now_unix_ms(),
            system,
            processes,
        }
    }

    /// Процесс по PID.
    pub fn process(&self, pid: u32) -> Option<&ProcessInfo> {
        self.processes.iter().find(|p| p.key.pid == pid)
    }

    /// Процесс по ключу.
    pub fn process_by_key(&self, key: &ProcessKey) -> Option<&ProcessInfo> {
        self.processes.iter().find(|p| p.key.same_process(key))
    }

    /// Детей процесса по PID.
    pub fn children_of(&self, pid: u32) -> Vec<&ProcessInfo> {
        self.processes
            .iter()
            .filter(|p| p.parent_pid == Some(pid))
            .collect()
    }
}

/// Текущее время в миллисекундах Unix.
pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_are_resolved_by_parent_pid() {
        let mut parent = ProcessInfo::new(ProcessKey::from_pid(1), "parent");
        parent.parent_pid = None;
        let mut child = ProcessInfo::new(ProcessKey::from_pid(2), "child");
        child.parent_pid = Some(1);

        let snapshot = Snapshot::new(SystemInfo::default(), vec![parent, child]);
        assert_eq!(snapshot.children_of(1).len(), 1);
        assert_eq!(snapshot.children_of(1)[0].name, "child");
        assert!(snapshot.children_of(2).is_empty());
    }

    #[test]
    fn snapshot_carries_schema_version() {
        let snapshot = Snapshot::new(SystemInfo::default(), Vec::new());
        assert_eq!(snapshot.schema_version, SCHEMA_VERSION);
    }
}
