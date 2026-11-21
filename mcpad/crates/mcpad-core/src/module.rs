//! Модули процесса: связи «процесс — загруженный модуль».
//!
//! Тип живёт в ядре, а не в canvas или в пробе: и проба, которая умеет
//! перечислять модули, и граф, который их рисует, должны говорить об одном и
//! том же.
//!
//! Поле [`ModuleLink::path`] — `None`, когда путь узнать не удалось. Это
//! «неизвестно», а не «пустой путь».

use serde::{Deserialize, Serialize};

/// Связь «процесс → загруженный модуль».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleLink {
    /// PID процесса, в котором найден модуль.
    pub pid: u32,
    /// Имя образа процесса.
    pub process_label: String,
    /// Имя модуля (имя файла).
    pub module_label: String,
    /// Полный путь к модулю, если его удалось получить.
    pub path: Option<String>,
}

impl ModuleLink {
    /// Связь без пути: путь неизвестен.
    pub fn new(
        pid: u32,
        process_label: impl Into<String>,
        module_label: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            pid,
            process_label: process_label.into(),
            module_label: module_label.into(),
            path,
        }
    }

    /// Имя файла модуля из полного пути.
    ///
    /// Для путей Windows используется и `\` и `/`, поэтому режем по обоим.
    pub fn file_name(path: &str) -> &str {
        let trimmed = path.trim_end_matches(['\\', '/']);
        match trimmed.rfind(['\\', '/']) {
            Some(index) => &trimmed[index + 1..],
            None => trimmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_handles_both_separators() {
        assert_eq!(ModuleLink::file_name("/usr/lib/libc.so.6"), "libc.so.6");
        assert_eq!(
            ModuleLink::file_name(r"C:\Windows\System32\ntdll.dll"),
            "ntdll.dll"
        );
        assert_eq!(ModuleLink::file_name("plain"), "plain");
        assert_eq!(ModuleLink::file_name("/usr/lib/"), "lib");
    }

    #[test]
    fn unknown_path_is_none() {
        let link = ModuleLink::new(10, "app", "core.dll", None);
        assert!(link.path.is_none());
        assert_eq!(link.pid, 10);
        assert_eq!(link.module_label, "core.dll");
    }
}
