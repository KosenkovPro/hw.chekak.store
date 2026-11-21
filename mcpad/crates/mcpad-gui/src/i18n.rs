//! Язык интерфейса: русский и английский.
//!
//! Строки лежат в одном словаре, а не разбросаны по коду: переключение языка
//! не должно требовать правок в вьюхах, а проверка «все строки переведены» —
//! выполняться чтением одной таблицы.
//!
//! Ключи не содержат данных: только смысл. Числа и имена подставляются на
//! месте, поэтому формат не зависит от языка.

use mcpad_series::TimeSpan;

/// Язык интерфейса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    /// Другой язык.
    pub fn toggle(self) -> Lang {
        match self {
            Lang::Ru => Lang::En,
            Lang::En => Lang::Ru,
        }
    }

    /// Код языка для командной строки.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    /// Язык по коду; неизвестный код — `None`.
    pub fn from_code(code: &str) -> Option<Lang> {
        match code.trim().to_ascii_lowercase().as_str() {
            "ru" => Some(Lang::Ru),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// Название языка в самом языке.
    pub fn self_name(self) -> &'static str {
        match self {
            Lang::Ru => "Русский",
            Lang::En => "English",
        }
    }
}

/// Ключи строк интерфейса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    ViewProcesses,
    ViewTree,
    ViewPerformance,
    ViewCanvas,
    ViewCapabilities,
    ColumnPid,
    ColumnName,
    ColumnCpu,
    ColumnMemory,
    ColumnThreads,
    ColumnHandles,
    ColumnUser,
    Search,
    Freeze,
    Frozen,
    NoSelection,
    ProcessGone,
    Collecting,
    Theme,
    ReloadTheme,
    ThemeLoaded,
    ThemeFailed,
    Dark,
    Light,
    Language,
    Window,
    CursorLeft,
    CursorRight,
    ChartSystemCpu,
    ChartMemory,
    ChartTopCpu,
    Statistics,
    Minimum,
    Maximum,
    Average,
    Gaps,
    Samples,
    ModeProcesses,
    ModeModules,
    ModulesUnavailable,
    ModulesOf,
    ZoomFit,
    ZoomReset,
    ExportSvg,
    ExportJson,
    Copied,
    Parent,
    Source,
    Confidence,
    NoData,
    Total,
    Unknown,
    Limitations,
    Descending,
}

/// Строки интерфейса для выбранного языка.
#[derive(Debug, Clone, Copy)]
pub struct Strings {
    pub lang: Lang,
}

impl Strings {
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// Строка по ключу.
    pub fn text(&self, key: Key) -> &'static str {
        match (self.lang, key) {
            (Lang::Ru, Key::ViewProcesses) => "Процессы",
            (Lang::En, Key::ViewProcesses) => "Processes",
            (Lang::Ru, Key::ViewTree) => "Дерево",
            (Lang::En, Key::ViewTree) => "Tree",
            (Lang::Ru, Key::ViewPerformance) => "Производительность",
            (Lang::En, Key::ViewPerformance) => "Performance",
            (Lang::Ru, Key::ViewCanvas) => "Canvas",
            (Lang::En, Key::ViewCanvas) => "Canvas",
            (Lang::Ru, Key::ViewCapabilities) => "Возможности",
            (Lang::En, Key::ViewCapabilities) => "Capabilities",
            (Lang::Ru, Key::ColumnPid) => "PID",
            (Lang::En, Key::ColumnPid) => "PID",
            (Lang::Ru, Key::ColumnName) => "Имя",
            (Lang::En, Key::ColumnName) => "Name",
            (Lang::Ru, Key::ColumnCpu) => "CPU%",
            (Lang::En, Key::ColumnCpu) => "CPU%",
            (Lang::Ru, Key::ColumnMemory) => "Память",
            (Lang::En, Key::ColumnMemory) => "Memory",
            (Lang::Ru, Key::ColumnThreads) => "Потоки",
            (Lang::En, Key::ColumnThreads) => "Threads",
            (Lang::Ru, Key::ColumnHandles) => "Дескрипторы",
            (Lang::En, Key::ColumnHandles) => "Handles",
            (Lang::Ru, Key::ColumnUser) => "Владелец",
            (Lang::En, Key::ColumnUser) => "User",
            (Lang::Ru, Key::Search) => "Поиск",
            (Lang::En, Key::Search) => "Search",
            (Lang::Ru, Key::Freeze) => "Заморозить (F6)",
            (Lang::En, Key::Freeze) => "Freeze (F6)",
            (Lang::Ru, Key::Frozen) => "Данные заморожены",
            (Lang::En, Key::Frozen) => "Data is frozen",
            (Lang::Ru, Key::NoSelection) => "Процесс не выбран",
            (Lang::En, Key::NoSelection) => "No process selected",
            (Lang::Ru, Key::ProcessGone) => "Процесс завершился",
            (Lang::En, Key::ProcessGone) => "Process has exited",
            (Lang::Ru, Key::Collecting) => "Собираем данные…",
            (Lang::En, Key::Collecting) => "Collecting data…",
            (Lang::Ru, Key::Theme) => "Тема",
            (Lang::En, Key::Theme) => "Theme",
            (Lang::Ru, Key::ReloadTheme) => "Перезагрузить тему",
            (Lang::En, Key::ReloadTheme) => "Reload theme",
            (Lang::Ru, Key::ThemeLoaded) => "Тема загружена",
            (Lang::En, Key::ThemeLoaded) => "Theme loaded",
            (Lang::Ru, Key::ThemeFailed) => "Тема не загружена",
            (Lang::En, Key::ThemeFailed) => "Theme not loaded",
            (Lang::Ru, Key::Dark) => "Тёмная",
            (Lang::En, Key::Dark) => "Dark",
            (Lang::Ru, Key::Light) => "Светлая",
            (Lang::En, Key::Light) => "Light",
            (Lang::Ru, Key::Language) => "Язык",
            (Lang::En, Key::Language) => "Language",
            (Lang::Ru, Key::Window) => "Окно",
            (Lang::En, Key::Window) => "Window",
            (Lang::Ru, Key::CursorLeft) => "◀",
            (Lang::En, Key::CursorLeft) => "◀",
            (Lang::Ru, Key::CursorRight) => "▶",
            (Lang::En, Key::CursorRight) => "▶",
            (Lang::Ru, Key::ChartSystemCpu) => "CPU системы",
            (Lang::En, Key::ChartSystemCpu) => "System CPU",
            (Lang::Ru, Key::ChartMemory) => "Занятая память",
            (Lang::En, Key::ChartMemory) => "Memory used",
            (Lang::Ru, Key::ChartTopCpu) => "Топ процессов по CPU",
            (Lang::En, Key::ChartTopCpu) => "Top processes by CPU",
            (Lang::Ru, Key::Statistics) => "Сводка",
            (Lang::En, Key::Statistics) => "Statistics",
            (Lang::Ru, Key::Minimum) => "Минимум",
            (Lang::En, Key::Minimum) => "Minimum",
            (Lang::Ru, Key::Maximum) => "Максимум",
            (Lang::En, Key::Maximum) => "Maximum",
            (Lang::Ru, Key::Average) => "Среднее",
            (Lang::En, Key::Average) => "Average",
            (Lang::Ru, Key::Gaps) => "Пропуски",
            (Lang::En, Key::Gaps) => "Gaps",
            (Lang::Ru, Key::Samples) => "Отсчёты",
            (Lang::En, Key::Samples) => "Samples",
            (Lang::Ru, Key::ModeProcesses) => "Процессы",
            (Lang::En, Key::ModeProcesses) => "Processes",
            (Lang::Ru, Key::ModeModules) => "Модули",
            (Lang::En, Key::ModeModules) => "Modules",
            (Lang::Ru, Key::ModulesUnavailable) => "Модули недоступны",
            (Lang::En, Key::ModulesUnavailable) => "Modules unavailable",
            (Lang::Ru, Key::ModulesOf) => "Модули процесса",
            (Lang::En, Key::ModulesOf) => "Modules of process",
            (Lang::Ru, Key::ZoomFit) => "Вписать",
            (Lang::En, Key::ZoomFit) => "Fit",
            (Lang::Ru, Key::ZoomReset) => "Сброс",
            (Lang::En, Key::ZoomReset) => "Reset",
            (Lang::Ru, Key::ExportSvg) => "Копировать SVG",
            (Lang::En, Key::ExportSvg) => "Copy SVG",
            (Lang::Ru, Key::ExportJson) => "Копировать JSON",
            (Lang::En, Key::ExportJson) => "Copy JSON",
            (Lang::Ru, Key::Copied) => "Скопировано в буфер",
            (Lang::En, Key::Copied) => "Copied to clipboard",
            (Lang::Ru, Key::Parent) => "Родитель",
            (Lang::En, Key::Parent) => "Parent",
            (Lang::Ru, Key::Source) => "Источник",
            (Lang::En, Key::Source) => "Source",
            (Lang::Ru, Key::Confidence) => "Уверенность",
            (Lang::En, Key::Confidence) => "Confidence",
            (Lang::Ru, Key::NoData) => "Нет данных",
            (Lang::En, Key::NoData) => "No data",
            (Lang::Ru, Key::Total) => "Всего",
            (Lang::En, Key::Total) => "Total",
            (Lang::Ru, Key::Unknown) => "неизвестно",
            (Lang::En, Key::Unknown) => "unknown",
            (Lang::Ru, Key::Limitations) => "Ограничения платформы",
            (Lang::En, Key::Limitations) => "Platform limitations",
            (Lang::Ru, Key::Descending) => "по убыванию",
            (Lang::En, Key::Descending) => "descending",
        }
    }

    /// Подпись окна времени.
    pub fn span_label(&self, span: TimeSpan) -> String {
        match (self.lang, span) {
            (Lang::Ru, TimeSpan::OneMinute) => "1 мин".to_string(),
            (Lang::Ru, TimeSpan::TwoMinutes) => "2 мин".to_string(),
            (Lang::Ru, TimeSpan::FiveMinutes) => "5 мин".to_string(),
            (Lang::En, TimeSpan::OneMinute) => "1 min".to_string(),
            (Lang::En, TimeSpan::TwoMinutes) => "2 min".to_string(),
            (Lang::En, TimeSpan::FiveMinutes) => "5 min".to_string(),
        }
    }
}

impl Default for Strings {
    fn default() -> Self {
        Strings::new(Lang::Ru)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_translation() {
        let keys = [
            Key::ViewProcesses,
            Key::ViewTree,
            Key::ViewPerformance,
            Key::ViewCanvas,
            Key::ViewCapabilities,
            Key::ColumnPid,
            Key::ColumnName,
            Key::ColumnCpu,
            Key::ColumnMemory,
            Key::ColumnThreads,
            Key::ColumnHandles,
            Key::ColumnUser,
            Key::Search,
            Key::Freeze,
            Key::Frozen,
            Key::NoSelection,
            Key::ProcessGone,
            Key::Collecting,
            Key::Theme,
            Key::ReloadTheme,
            Key::ThemeLoaded,
            Key::ThemeFailed,
            Key::Dark,
            Key::Light,
            Key::Language,
            Key::Window,
            Key::CursorLeft,
            Key::CursorRight,
            Key::ChartSystemCpu,
            Key::ChartMemory,
            Key::ChartTopCpu,
            Key::Statistics,
            Key::Minimum,
            Key::Maximum,
            Key::Average,
            Key::Gaps,
            Key::Samples,
            Key::ModeProcesses,
            Key::ModeModules,
            Key::ModulesUnavailable,
            Key::ModulesOf,
            Key::ZoomFit,
            Key::ZoomReset,
            Key::ExportSvg,
            Key::ExportJson,
            Key::Copied,
            Key::Parent,
            Key::Source,
            Key::Confidence,
            Key::NoData,
            Key::Total,
            Key::Unknown,
            Key::Limitations,
            Key::Descending,
        ];

        let ru = Strings::new(Lang::Ru);
        let en = Strings::new(Lang::En);
        for key in keys {
            assert!(!ru.text(key).is_empty(), "пустая строка RU: {key:?}");
            assert!(!en.text(key).is_empty(), "пустая строка EN: {key:?}");
        }
    }

    #[test]
    fn translations_differ_where_it_matters() {
        let ru = Strings::new(Lang::Ru);
        let en = Strings::new(Lang::En);
        assert_ne!(ru.text(Key::ViewProcesses), en.text(Key::ViewProcesses));
        assert_ne!(ru.text(Key::Search), en.text(Key::Search));
    }

    #[test]
    fn language_toggles() {
        assert_eq!(Lang::Ru.toggle(), Lang::En);
        assert_eq!(Lang::En.toggle(), Lang::Ru);
    }

    #[test]
    fn language_code_round_trips() {
        assert_eq!(Lang::from_code("ru"), Some(Lang::Ru));
        assert_eq!(Lang::from_code("EN"), Some(Lang::En));
        assert_eq!(Lang::from_code("de"), None);
    }

    #[test]
    fn span_labels_are_localized() {
        let ru = Strings::new(Lang::Ru);
        let en = Strings::new(Lang::En);
        assert!(ru.span_label(TimeSpan::OneMinute).contains("мин"));
        assert!(en.span_label(TimeSpan::OneMinute).contains("min"));
    }
}
