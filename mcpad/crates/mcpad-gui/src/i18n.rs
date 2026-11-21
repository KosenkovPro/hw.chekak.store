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
    /// Русский.
    Ru,
    /// Английский.
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
    /// Представление «Процессы».
    ViewProcesses,
    /// Представление «Дерево».
    ViewTree,
    /// Представление «Производительность».
    ViewPerformance,
    /// Представление «Canvas».
    ViewCanvas,
    /// Представление «Возможности».
    ViewCapabilities,
    /// Колонка PID.
    ColumnPid,
    /// Колонка имени.
    ColumnName,
    /// Колонка CPU.
    ColumnCpu,
    /// Колонка памяти.
    ColumnMemory,
    /// Колонка приватной памяти.
    ColumnPrivateMemory,
    /// Колонка потоков.
    ColumnThreads,
    /// Колонка дескрипторов.
    ColumnHandles,
    /// Колонка чтения.
    ColumnIoRead,
    /// Колонка записи.
    ColumnIoWrite,
    /// Колонка мини-графика.
    ColumnHistory,
    /// Колонка владельца.
    ColumnUser,
    /// Поиск.
    Search,
    /// Подсказка в поле поиска.
    SearchHint,
    /// Заморозить данные.
    Freeze,
    /// Заморозить — короткая подпись.
    FreezeShort,
    /// Данные заморожены — короткая подпись.
    FrozenShort,
    /// Данные заморожены.
    Frozen,
    /// Процесс не выбран.
    NoSelection,
    /// Процесс завершился.
    ProcessGone,
    /// Идёт сбор данных.
    Collecting,
    /// Тема.
    Theme,
    /// Перезагрузить тему.
    ReloadTheme,
    /// Тема загружена.
    ThemeLoaded,
    /// Тема не загружена.
    ThemeFailed,
    /// Тёмная тема.
    Dark,
    /// Светлая тема.
    Light,
    /// Язык.
    Language,
    /// Окно времени.
    Window,
    /// Курсор влево.
    CursorLeft,
    /// Курсор вправо.
    CursorRight,
    /// График CPU системы.
    ChartSystemCpu,
    /// График памяти.
    ChartMemory,
    /// Топ процессов по CPU.
    ChartTopCpu,
    /// Сводка.
    Statistics,
    /// Минимум.
    Minimum,
    /// Максимум.
    Maximum,
    /// Среднее.
    Average,
    /// Пропуски.
    Gaps,
    /// Отсчёты.
    Samples,
    /// Режим «Процессы».
    ModeProcesses,
    /// Режим «Модули».
    ModeModules,
    /// Модули недоступны.
    ModulesUnavailable,
    /// Модули процесса.
    ModulesOf,
    /// Вписать.
    ZoomFit,
    /// Сброс.
    ZoomReset,
    /// Копировать SVG.
    ExportSvg,
    /// Копировать JSON.
    ExportJson,
    /// Скопировано.
    Copied,
    /// Родительский процесс.
    Parent,
    /// Источник данных.
    Source,
    /// Уверенность.
    Confidence,
    /// Нет данных.
    NoData,
    /// Всего.
    Total,
    /// Неизвестно.
    Unknown,
    /// Ограничения платформы.
    Limitations,
    /// Сортировка по убыванию.
    Descending,
    /// Выбор столбцов.
    Columns,
    /// Панель сведений.
    Details,
    /// Подсказка к панели сведений.
    DetailsPanel,
    /// Время снимка.
    Snapshot,
    /// Интервал обновления.
    Interval,
    /// Миллисекунды.
    Milliseconds,
    /// Секунды.
    Seconds,
    /// Раздел ресурсов.
    Resources,
    /// Раздел сведений о процессе.
    ProcessInfo,
    /// Путь к образу.
    Path,
    /// Командная строка.
    CommandLine,
    /// Процесс защищён системой.
    Protected,
    /// Процесс не защищён.
    NotProtected,
    /// Состояние защиты неизвестно.
    ProtectionUnknown,
    /// Раздел происхождения данных.
    DataProvenance,
    /// Плотность таблицы.
    Density,
    /// Компактные строки.
    Compact,
    /// Обычные строки.
    Comfortable,
    /// Загрузка системы.
    SystemLoad,
    /// Занято памяти.
    MemoryUsed,
    /// Всего потоков.
    TotalThreads,
    /// Всего дескрипторов.
    TotalHandles,
    /// Загрузка CPU.
    CpuLoad,
    /// Свободная память.
    MemoryFree,
    /// Узел.
    Host,
    /// Горячие клавиши.
    Shortcuts,
    /// Очистить поиск.
    ClearSearch,
    /// Переключение представлений горячими клавишами.
    ViewShortcuts,
    /// Подсказка выбрать процесс.
    SelectProcessHint,
    /// Возможность поддерживается полностью.
    SupportFull,
    /// Возможность поддерживается частично.
    SupportPartial,
    /// Возможность не поддерживается.
    SupportNone,
    /// Группировка процессов.
    GroupBy,
    /// Без группировки.
    GroupNone,
    /// Группировка по приложению.
    GroupApplication,
    /// Группировка по родителю.
    GroupParent,
    /// Экспорт в CSV.
    ExportCsv,
    /// Развернуть всё.
    ExpandAll,
    /// Свернуть всё.
    CollapseAll,
    /// Скрыть системные процессы.
    HideSystem,
    /// Быстрый фильтр: высокая загрузка CPU.
    FilterHighCpu,
    /// Быстрый фильтр: высокая память.
    FilterHighMemory,
    /// Сколько строк показано.
    Matched,
    /// Модули процесса.
    Modules,
    /// Скопировать PID.
    CopyPid,
    /// Скопировать имя.
    CopyName,
    /// Скопировать путь.
    CopyPath,
    /// Профиль столбцов: базовые.
    ProfileBasic,
    /// Профиль столбцов: CPU.
    ProfileCpu,
    /// Профиль столбцов: память.
    ProfileMemory,
    /// Профиль столбцов: ввод-вывод.
    ProfileIo,
    /// Завершить процесс.
    Terminate,
    /// Подтверждение завершения.
    TerminateConfirm,
    /// Процесс завершён.
    Terminated,
    /// Не удалось завершить процесс.
    TerminateFailed,
    /// Приоритет процесса.
    Priority,
    /// Приоритет: низкий.
    PriorityLow,
    /// Приоритет: ниже обычного.
    PriorityBelowNormal,
    /// Приоритет: обычный.
    PriorityNormal,
    /// Приоритет: выше обычного.
    PriorityAboveNormal,
    /// Приоритет: высокий.
    PriorityHigh,
    /// Приоритет: реального времени.
    PriorityRealTime,
    /// Приоритет изменён.
    PriorityChanged,
    /// Процесс защищён системой.
    ProtectedProcess,
    /// Режим поиска.
    FilterMode,
    /// Режим поиска: обычный текст.
    FilterModeText,
    /// Режим поиска: регулярное выражение.
    FilterModeRegex,
    /// Некорректное выражение фильтра.
    InvalidFilter,
}

/// Строки интерфейса для выбранного языка.
#[derive(Debug, Clone, Copy)]
pub struct Strings {
    /// Выбранный язык.
    pub lang: Lang,
}

impl Strings {
    /// Строки для языка.
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
            (Lang::Ru, Key::ColumnCpu) => "CPU %",
            (Lang::En, Key::ColumnCpu) => "CPU %",
            (Lang::Ru, Key::ColumnMemory) => "Память, МБ",
            (Lang::En, Key::ColumnMemory) => "Memory, MB",
            (Lang::Ru, Key::ColumnPrivateMemory) => "Приватная, МБ",
            (Lang::En, Key::ColumnPrivateMemory) => "Private, MB",
            (Lang::Ru, Key::ColumnThreads) => "Потоки",
            (Lang::En, Key::ColumnThreads) => "Threads",
            (Lang::Ru, Key::ColumnHandles) => "Дескрипторы",
            (Lang::En, Key::ColumnHandles) => "Handles",
            (Lang::Ru, Key::ColumnIoRead) => "Чтение",
            (Lang::En, Key::ColumnIoRead) => "Read",
            (Lang::Ru, Key::ColumnIoWrite) => "Запись",
            (Lang::En, Key::ColumnIoWrite) => "Write",
            (Lang::Ru, Key::ColumnHistory) => "История CPU",
            (Lang::En, Key::ColumnHistory) => "CPU history",
            (Lang::Ru, Key::ColumnUser) => "Владелец",
            (Lang::En, Key::ColumnUser) => "User",
            (Lang::Ru, Key::Search) => "Поиск",
            (Lang::En, Key::Search) => "Search",
            (Lang::Ru, Key::SearchHint) => "Найти процесс…",
            (Lang::En, Key::SearchHint) => "Find process…",
            (Lang::Ru, Key::Freeze) => "Заморозить (F6)",
            (Lang::En, Key::Freeze) => "Freeze (F6)",
            (Lang::Ru, Key::FreezeShort) => "Заморозить",
            (Lang::En, Key::FreezeShort) => "Freeze",
            (Lang::Ru, Key::FrozenShort) => "Заморожено",
            (Lang::En, Key::FrozenShort) => "Frozen",
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
            (Lang::Ru, Key::Columns) => "Столбцы",
            (Lang::En, Key::Columns) => "Columns",
            (Lang::Ru, Key::Details) => "Сведения",
            (Lang::En, Key::Details) => "Details",
            (Lang::Ru, Key::DetailsPanel) => "Показать или скрыть панель сведений",
            (Lang::En, Key::DetailsPanel) => "Show or hide the details panel",
            (Lang::Ru, Key::Snapshot) => "Снимок",
            (Lang::En, Key::Snapshot) => "Snapshot",
            (Lang::Ru, Key::Interval) => "Интервал",
            (Lang::En, Key::Interval) => "Interval",
            (Lang::Ru, Key::Milliseconds) => "мс",
            (Lang::En, Key::Milliseconds) => "ms",
            (Lang::Ru, Key::Seconds) => "с",
            (Lang::En, Key::Seconds) => "s",
            (Lang::Ru, Key::Resources) => "Ресурсы",
            (Lang::En, Key::Resources) => "Resources",
            (Lang::Ru, Key::ProcessInfo) => "Процесс",
            (Lang::En, Key::ProcessInfo) => "Process",
            (Lang::Ru, Key::Path) => "Путь",
            (Lang::En, Key::Path) => "Path",
            (Lang::Ru, Key::CommandLine) => "Командная строка",
            (Lang::En, Key::CommandLine) => "Command line",
            (Lang::Ru, Key::Protected) => "Защищён системой",
            (Lang::En, Key::Protected) => "Protected by system",
            (Lang::Ru, Key::NotProtected) => "Не защищён",
            (Lang::En, Key::NotProtected) => "Not protected",
            (Lang::Ru, Key::ProtectionUnknown) => "Защита неизвестна",
            (Lang::En, Key::ProtectionUnknown) => "Protection unknown",
            (Lang::Ru, Key::DataProvenance) => "Происхождение данных",
            (Lang::En, Key::DataProvenance) => "Data provenance",
            (Lang::Ru, Key::Density) => "Плотность",
            (Lang::En, Key::Density) => "Density",
            (Lang::Ru, Key::Compact) => "Компактно",
            (Lang::En, Key::Compact) => "Compact",
            (Lang::Ru, Key::Comfortable) => "Обычно",
            (Lang::En, Key::Comfortable) => "Comfortable",
            (Lang::Ru, Key::SystemLoad) => "Загрузка",
            (Lang::En, Key::SystemLoad) => "Load",
            (Lang::Ru, Key::MemoryUsed) => "Занято памяти",
            (Lang::En, Key::MemoryUsed) => "Memory used",
            (Lang::Ru, Key::TotalThreads) => "Всего потоков",
            (Lang::En, Key::TotalThreads) => "Total threads",
            (Lang::Ru, Key::TotalHandles) => "Всего дескрипторов",
            (Lang::En, Key::TotalHandles) => "Total handles",
            (Lang::Ru, Key::CpuLoad) => "Загрузка CPU",
            (Lang::En, Key::CpuLoad) => "CPU load",
            (Lang::Ru, Key::MemoryFree) => "Свободно памяти",
            (Lang::En, Key::MemoryFree) => "Memory free",
            (Lang::Ru, Key::Host) => "Узел",
            (Lang::En, Key::Host) => "Host",
            (Lang::Ru, Key::Shortcuts) => "Горячие клавиши",
            (Lang::En, Key::Shortcuts) => "Shortcuts",
            (Lang::Ru, Key::ClearSearch) => "Очистить",
            (Lang::En, Key::ClearSearch) => "Clear",
            (Lang::Ru, Key::ViewShortcuts) => "Переключение раздела",
            (Lang::En, Key::ViewShortcuts) => "Switch section",
            (Lang::Ru, Key::SelectProcessHint) => {
                "Выберите процесс в списке — здесь появятся его путь, командная строка и владелец."
            }
            (Lang::En, Key::SelectProcessHint) => {
                "Select a process in the list to see its path, command line and owner."
            }
            (Lang::Ru, Key::SupportFull) => "полностью",
            (Lang::En, Key::SupportFull) => "full",
            (Lang::Ru, Key::SupportPartial) => "частично",
            (Lang::En, Key::SupportPartial) => "partial",
            (Lang::Ru, Key::SupportNone) => "не поддерживается",
            (Lang::En, Key::SupportNone) => "unsupported",
            (Lang::Ru, Key::GroupBy) => "Группировка",
            (Lang::En, Key::GroupBy) => "Group by",
            (Lang::Ru, Key::GroupNone) => "Без группировки",
            (Lang::En, Key::GroupNone) => "No grouping",
            (Lang::Ru, Key::GroupApplication) => "По приложению",
            (Lang::En, Key::GroupApplication) => "By application",
            (Lang::Ru, Key::GroupParent) => "По родителю",
            (Lang::En, Key::GroupParent) => "By parent",
            (Lang::Ru, Key::ExportCsv) => "Копировать CSV",
            (Lang::En, Key::ExportCsv) => "Copy CSV",
            (Lang::Ru, Key::ExpandAll) => "Развернуть всё",
            (Lang::En, Key::ExpandAll) => "Expand all",
            (Lang::Ru, Key::CollapseAll) => "Свернуть всё",
            (Lang::En, Key::CollapseAll) => "Collapse all",
            (Lang::Ru, Key::HideSystem) => "Скрыть системные",
            (Lang::En, Key::HideSystem) => "Hide system",
            (Lang::Ru, Key::FilterHighCpu) => "Высокая загрузка CPU",
            (Lang::En, Key::FilterHighCpu) => "High CPU",
            (Lang::Ru, Key::FilterHighMemory) => "Высокая память",
            (Lang::En, Key::FilterHighMemory) => "High memory",
            (Lang::Ru, Key::Matched) => "Показано",
            (Lang::En, Key::Matched) => "Shown",
            (Lang::Ru, Key::Modules) => "Модули",
            (Lang::En, Key::Modules) => "Modules",
            (Lang::Ru, Key::CopyPid) => "Скопировать PID",
            (Lang::En, Key::CopyPid) => "Copy PID",
            (Lang::Ru, Key::CopyName) => "Скопировать имя",
            (Lang::En, Key::CopyName) => "Copy name",
            (Lang::Ru, Key::CopyPath) => "Скопировать путь",
            (Lang::En, Key::CopyPath) => "Copy path",
            (Lang::Ru, Key::ProfileBasic) => "Базовые",
            (Lang::En, Key::ProfileBasic) => "Basic",
            (Lang::Ru, Key::ProfileCpu) => "CPU",
            (Lang::En, Key::ProfileCpu) => "CPU",
            (Lang::Ru, Key::ProfileMemory) => "Память",
            (Lang::En, Key::ProfileMemory) => "Memory",
            (Lang::Ru, Key::ProfileIo) => "Ввод-вывод",
            (Lang::En, Key::ProfileIo) => "I/O",
            (Lang::Ru, Key::Terminate) => "Снять задачу",
            (Lang::En, Key::Terminate) => "End task",
            (Lang::Ru, Key::TerminateConfirm) => "Точно снять задачу?",
            (Lang::En, Key::TerminateConfirm) => "End task for sure?",
            (Lang::Ru, Key::Terminated) => "Процесс завершён",
            (Lang::En, Key::Terminated) => "Process ended",
            (Lang::Ru, Key::TerminateFailed) => "Не удалось снять задачу",
            (Lang::En, Key::TerminateFailed) => "Could not end the task",
            (Lang::Ru, Key::Priority) => "Приоритет",
            (Lang::En, Key::Priority) => "Priority",
            (Lang::Ru, Key::PriorityLow) => "Низкий",
            (Lang::En, Key::PriorityLow) => "Low",
            (Lang::Ru, Key::PriorityBelowNormal) => "Ниже обычного",
            (Lang::En, Key::PriorityBelowNormal) => "Below normal",
            (Lang::Ru, Key::PriorityNormal) => "Обычный",
            (Lang::En, Key::PriorityNormal) => "Normal",
            (Lang::Ru, Key::PriorityAboveNormal) => "Выше обычного",
            (Lang::En, Key::PriorityAboveNormal) => "Above normal",
            (Lang::Ru, Key::PriorityHigh) => "Высокий",
            (Lang::En, Key::PriorityHigh) => "High",
            (Lang::Ru, Key::PriorityRealTime) => "Реального времени",
            (Lang::En, Key::PriorityRealTime) => "Real time",
            (Lang::Ru, Key::PriorityChanged) => "Приоритет изменён",
            (Lang::En, Key::PriorityChanged) => "Priority changed",
            (Lang::Ru, Key::ProtectedProcess) => "Процесс защищён системой",
            (Lang::En, Key::ProtectedProcess) => "Process is protected by the system",
            (Lang::Ru, Key::FilterMode) => "Режим поиска",
            (Lang::En, Key::FilterMode) => "Search mode",
            (Lang::Ru, Key::FilterModeText) => "Текст",
            (Lang::En, Key::FilterModeText) => "Text",
            (Lang::Ru, Key::FilterModeRegex) => "Регулярное выражение",
            (Lang::En, Key::FilterModeRegex) => "Regex",
            (Lang::Ru, Key::InvalidFilter) => "Некорректное выражение",
            (Lang::En, Key::InvalidFilter) => "Invalid expression",
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

/// Все ключи интерфейса — таблица для проверки полноты перевода.
pub const ALL_KEYS: [Key; 129] = [
    Key::ViewProcesses,
    Key::ViewTree,
    Key::ViewPerformance,
    Key::ViewCanvas,
    Key::ViewCapabilities,
    Key::ColumnPid,
    Key::ColumnName,
    Key::ColumnCpu,
    Key::ColumnMemory,
    Key::ColumnPrivateMemory,
    Key::ColumnThreads,
    Key::ColumnHandles,
    Key::ColumnIoRead,
    Key::ColumnIoWrite,
    Key::ColumnHistory,
    Key::ColumnUser,
    Key::Search,
    Key::SearchHint,
    Key::Freeze,
    Key::FreezeShort,
    Key::FrozenShort,
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
    Key::Columns,
    Key::Details,
    Key::DetailsPanel,
    Key::Snapshot,
    Key::Interval,
    Key::Milliseconds,
    Key::Seconds,
    Key::Resources,
    Key::ProcessInfo,
    Key::Path,
    Key::CommandLine,
    Key::Protected,
    Key::NotProtected,
    Key::ProtectionUnknown,
    Key::DataProvenance,
    Key::Density,
    Key::Compact,
    Key::Comfortable,
    Key::SystemLoad,
    Key::MemoryUsed,
    Key::TotalThreads,
    Key::TotalHandles,
    Key::CpuLoad,
    Key::MemoryFree,
    Key::Host,
    Key::Shortcuts,
    Key::ClearSearch,
    Key::ViewShortcuts,
    Key::SelectProcessHint,
    Key::SupportFull,
    Key::SupportPartial,
    Key::SupportNone,
    Key::GroupBy,
    Key::GroupNone,
    Key::GroupApplication,
    Key::GroupParent,
    Key::ExportCsv,
    Key::ExpandAll,
    Key::CollapseAll,
    Key::HideSystem,
    Key::FilterHighCpu,
    Key::FilterHighMemory,
    Key::Matched,
    Key::Modules,
    Key::CopyPid,
    Key::CopyName,
    Key::CopyPath,
    Key::ProfileBasic,
    Key::ProfileCpu,
    Key::ProfileMemory,
    Key::ProfileIo,
    Key::Terminate,
    Key::TerminateConfirm,
    Key::Terminated,
    Key::TerminateFailed,
    Key::Priority,
    Key::PriorityLow,
    Key::PriorityBelowNormal,
    Key::PriorityNormal,
    Key::PriorityAboveNormal,
    Key::PriorityHigh,
    Key::PriorityRealTime,
    Key::PriorityChanged,
    Key::ProtectedProcess,
    Key::FilterMode,
    Key::FilterModeText,
    Key::FilterModeRegex,
    Key::InvalidFilter,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_translation() {
        let ru = Strings::new(Lang::Ru);
        let en = Strings::new(Lang::En);
        for key in ALL_KEYS {
            assert!(!ru.text(key).is_empty(), "пустая строка RU: {key:?}");
            assert!(!en.text(key).is_empty(), "пустая строка EN: {key:?}");
        }
    }

    #[test]
    fn key_table_has_no_duplicates() {
        let mut sorted = ALL_KEYS;
        sorted.sort_by_key(|key| *key as u8);
        for pair in sorted.windows(2) {
            assert_ne!(pair[0], pair[1], "ключ повторяется в таблице");
        }
    }

    #[test]
    fn translations_differ_where_it_matters() {
        let ru = Strings::new(Lang::Ru);
        let en = Strings::new(Lang::En);
        assert_ne!(ru.text(Key::ViewProcesses), en.text(Key::ViewProcesses));
        assert_ne!(ru.text(Key::Search), en.text(Key::Search));
        assert_ne!(ru.text(Key::ColumnMemory), en.text(Key::ColumnMemory));
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
