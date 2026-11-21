//! Колонки таблицы процессов.
//!
//! Колонка — это единое описание: заголовок, ширина, признак «числовая»,
//! значение для сортировки, текст в ячейке и доля нагрузки для тепловой
//! карты. Всё в одном месте, чтобы таблица не превратилась в набор разрозненных
//! `if`.
//!
//! Правило сортировки: **неизвестное значение всегда в конце** — и при
//! убывании тоже. Иначе процессы без данных оказывались бы наверху при
//! сортировке по убыванию и выглядели бы «самыми нагруженными».

use mcpad_core::ProcessInfo;

use crate::i18n::{Key, Strings};

/// Профиль набора столбцов.
///
/// Профили есть и в RustWTM («Basic», «CPU», «Memory», «I/O»): переключение
/// одним щелчком вместо выключения колонок по одной.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Базовый набор: как в Диспетчере задач.
    Basic,
    /// Загрузка CPU и её история.
    Cpu,
    /// Память: рабочий набор и приватная.
    Memory,
    /// Ввод-вывод.
    Io,
}

impl Profile {
    /// Все профили.
    pub fn all() -> [Profile; 4] {
        [Profile::Basic, Profile::Cpu, Profile::Memory, Profile::Io]
    }

    /// Столбцы профиля.
    pub fn columns(self) -> Vec<Column> {
        match self {
            Profile::Basic => vec![
                Column::Pid,
                Column::Name,
                Column::Cpu,
                Column::Memory,
                Column::Threads,
                Column::Handles,
                Column::History,
            ],
            Profile::Cpu => vec![Column::Pid, Column::Name, Column::Cpu, Column::History],
            Profile::Memory => vec![
                Column::Pid,
                Column::Name,
                Column::Memory,
                Column::PrivateMemory,
                Column::Cpu,
                Column::History,
            ],
            Profile::Io => vec![
                Column::Pid,
                Column::Name,
                Column::IoRead,
                Column::IoWrite,
                Column::Cpu,
                Column::History,
            ],
        }
    }
}

/// Колонка таблицы процессов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// Идентификатор процесса.
    Pid,
    /// Имя образа.
    Name,
    /// Доля CPU в процентах от всей машины.
    Cpu,
    /// Working set.
    Memory,
    /// Приватная память.
    PrivateMemory,
    /// Число потоков.
    Threads,
    /// Число дескрипторов.
    Handles,
    /// Прочитано из файлов и устройств.
    IoRead,
    /// Записано в файлы и устройства.
    IoWrite,
    /// Мини-график доли CPU.
    History,
}

/// Значение колонки для сортировки.
#[derive(Debug, Clone, PartialEq)]
pub enum SortValue {
    /// Число.
    Number(f64),
    /// Текст.
    Text(String),
}

impl SortValue {
    /// Сравнивает два значения; `None` — «значения нет».
    ///
    /// Значение без данных всегда уходит в конец списка, независимо от
    /// направления сортировки.
    pub fn compare(left: Option<&SortValue>, right: Option<&SortValue>) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (left, right) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => Self::compare_values(left, right),
        }
    }

    /// Сравнивает сами значения, без учёта наличия.
    ///
    /// Направление сортировки применяется только здесь: иначе «значения нет»
    /// поменялось бы местами со «значение есть» и пропуски всплыли бы наверх.
    pub fn compare_values(left: &SortValue, right: &SortValue) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (left, right) {
            (SortValue::Number(left), SortValue::Number(right)) => left.total_cmp(right),
            (SortValue::Text(left), SortValue::Text(right)) => left.cmp(right),
            _ => Ordering::Equal,
        }
    }
}

impl Column {
    /// Все колонки в порядке показа.
    pub fn all() -> [Column; 10] {
        [
            Column::Pid,
            Column::Name,
            Column::Cpu,
            Column::Memory,
            Column::PrivateMemory,
            Column::Threads,
            Column::Handles,
            Column::IoRead,
            Column::IoWrite,
            Column::History,
        ]
    }

    /// Колонки, включённые по умолчанию.
    pub fn default_visible() -> [Column; 7] {
        [
            Column::Pid,
            Column::Name,
            Column::Cpu,
            Column::Memory,
            Column::Threads,
            Column::Handles,
            Column::History,
        ]
    }

    /// Заголовок колонки.
    pub fn title(self, strings: Strings) -> &'static str {
        match self {
            Column::Pid => strings.text(Key::ColumnPid),
            Column::Name => strings.text(Key::ColumnName),
            Column::Cpu => strings.text(Key::ColumnCpu),
            Column::Memory => strings.text(Key::ColumnMemory),
            Column::PrivateMemory => strings.text(Key::ColumnPrivateMemory),
            Column::Threads => strings.text(Key::ColumnThreads),
            Column::Handles => strings.text(Key::ColumnHandles),
            Column::IoRead => strings.text(Key::ColumnIoRead),
            Column::IoWrite => strings.text(Key::ColumnIoWrite),
            Column::History => strings.text(Key::ColumnHistory),
        }
    }

    /// Ширина колонки в пикселях.
    pub fn width(self) -> f32 {
        match self {
            Column::Pid => 64.0,
            Column::Name => 240.0,
            Column::Cpu => 76.0,
            Column::Memory => 92.0,
            Column::PrivateMemory => 92.0,
            Column::Threads => 76.0,
            Column::Handles => 88.0,
            Column::IoRead => 92.0,
            Column::IoWrite => 92.0,
            Column::History => 132.0,
        }
    }

    /// Сортируется ли колонка по числу.
    pub fn numeric(self) -> bool {
        !matches!(self, Column::Name | Column::History)
    }

    /// Значение для сортировки.
    pub fn value(self, process: &ProcessInfo) -> Option<SortValue> {
        let number = match self {
            Column::Pid => process.key.pid as f64,
            Column::Cpu => process.cpu_percent.value?,
            Column::Memory => process.memory_working_set.value? as f64,
            Column::PrivateMemory => process.memory_private.value? as f64,
            Column::Threads => process.thread_count.value? as f64,
            Column::Handles => process.handle_count.value? as f64,
            Column::IoRead => process.io_read_bytes.value? as f64,
            Column::IoWrite => process.io_write_bytes.value? as f64,
            Column::Name => {
                return Some(SortValue::Text(process.name.to_lowercase()));
            }
            Column::History => return None,
        };
        Some(SortValue::Number(number))
    }

    /// Доля нагрузки для тепловой карты: `None`, если красить нечего.
    pub fn heat_ratio(self, process: &ProcessInfo, memory_total: Option<u64>) -> Option<f64> {
        match self {
            Column::Cpu => process.cpu_percent.value.map(|value| value / 100.0),
            Column::Memory => {
                let used = process.memory_working_set.value? as f64;
                let total = memory_total? as f64;
                if total <= 0.0 {
                    return None;
                }
                Some(used / total)
            }
            Column::PrivateMemory => {
                let used = process.memory_private.value? as f64;
                let total = memory_total? as f64;
                if total <= 0.0 {
                    return None;
                }
                Some(used / total)
            }
            _ => None,
        }
    }

    /// Текст в ячейке; `None` — значение неизвестно.
    pub fn cell(self, process: &ProcessInfo) -> Option<String> {
        match self {
            Column::Pid => Some(process.key.pid.to_string()),
            Column::Name => Some(process.name.clone()),
            Column::Cpu => process.cpu_percent.value.map(|value| format!("{value:.1}")),
            Column::Memory => process
                .memory_working_set
                .value
                .map(|value| format!("{:.1}", value as f64 / 1_048_576.0)),
            Column::PrivateMemory => process
                .memory_private
                .value
                .map(|value| format!("{:.1}", value as f64 / 1_048_576.0)),
            Column::Threads => process.thread_count.value.map(|value| value.to_string()),
            Column::Handles => process.handle_count.value.map(|value| value.to_string()),
            Column::IoRead => process.io_read_bytes.value.map(format_bytes),
            Column::IoWrite => process.io_write_bytes.value.map(format_bytes),
            Column::History => None,
        }
    }

    /// Выравнивание по горизонтали.
    pub fn align(self) -> egui::Align {
        match self {
            Column::Name | Column::History => egui::Align::LEFT,
            _ => egui::Align::RIGHT,
        }
    }
}

/// Форматирует объём байтов с автоматическим выбором единицы.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let value = bytes as f64;
    if value < KIB {
        format!("{bytes} Б")
    } else if value < MIB {
        format!("{:.1} КБ", value / KIB)
    } else if value < GIB {
        format!("{:.1} МБ", value / MIB)
    } else {
        format!("{:.1} ГБ", value / GIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey, Source};

    fn process() -> ProcessInfo {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(42), "example.exe");
        process.cpu_percent = Counter::measured(12.5, Source::Derived);
        process.memory_working_set =
            Counter::measured(64 * 1_048_576, Source::WindowsSystemInformation);
        process.memory_private =
            Counter::measured(32 * 1_048_576, Source::WindowsSystemInformation);
        process.thread_count = Counter::measured(7, Source::WindowsSystemInformation);
        process.handle_count = Counter::measured(11, Source::WindowsSystemInformation);
        process.io_read_bytes = Counter::measured(2048, Source::WindowsSystemInformation);
        process.io_write_bytes = Counter::measured(3 * 1_048_576, Source::WindowsSystemInformation);
        process
    }

    #[test]
    fn known_values_are_available() {
        let process = process();
        assert_eq!(Column::Cpu.cell(&process).as_deref(), Some("12.5"));
        assert_eq!(Column::Memory.cell(&process).as_deref(), Some("64.0"));
        assert_eq!(
            Column::PrivateMemory.cell(&process).as_deref(),
            Some("32.0")
        );
        assert_eq!(Column::Threads.cell(&process).as_deref(), Some("7"));
        assert_eq!(Column::Pid.cell(&process).as_deref(), Some("42"));
    }

    #[test]
    fn unknown_values_have_no_text() {
        let process = ProcessInfo::new(ProcessKey::from_pid(1), "empty.exe");
        assert_eq!(Column::Cpu.cell(&process), None);
        assert_eq!(Column::Memory.cell(&process), None);
        assert_eq!(Column::Cpu.value(&process), None);
    }

    #[test]
    fn history_column_is_not_sortable() {
        assert_eq!(Column::History.value(&process()), None);
        assert!(!Column::History.numeric());
    }

    #[test]
    fn missing_values_sort_last_in_both_directions() {
        use std::cmp::Ordering;
        let known = Some(SortValue::Number(1.0));
        let missing: Option<SortValue> = None;
        assert_eq!(
            SortValue::compare(missing.as_ref(), known.as_ref()),
            Ordering::Greater
        );
        assert_eq!(
            SortValue::compare(known.as_ref(), missing.as_ref()),
            Ordering::Less
        );
    }

    #[test]
    fn numbers_compare_numerically() {
        use std::cmp::Ordering;
        let small = Some(SortValue::Number(2.0));
        let big = Some(SortValue::Number(10.0));
        assert_eq!(
            SortValue::compare(small.as_ref(), big.as_ref()),
            Ordering::Less
        );
        assert_eq!(
            SortValue::compare(big.as_ref(), small.as_ref()),
            Ordering::Greater
        );
    }

    #[test]
    fn value_comparison_ignores_presence() {
        use std::cmp::Ordering;
        let low = SortValue::Number(1.0);
        let high = SortValue::Number(2.0);
        assert_eq!(SortValue::compare_values(&low, &high), Ordering::Less);
    }

    #[test]
    fn cpu_heat_ratio_is_share_of_machine() {
        let process = process();
        let ratio = Column::Cpu.heat_ratio(&process, None).expect("доля есть");
        assert!((ratio - 0.125).abs() < 1e-9);
    }

    #[test]
    fn memory_heat_ratio_needs_total_memory() {
        let process = process();
        assert_eq!(Column::Memory.heat_ratio(&process, None), None);
        let ratio = Column::Memory
            .heat_ratio(&process, Some(256 * 1_048_576))
            .expect("доля есть");
        assert!((ratio - 0.25).abs() < 1e-9);
    }

    #[test]
    fn bytes_are_formatted_with_unit() {
        assert_eq!(format_bytes(512), "512 Б");
        assert_eq!(format_bytes(2048), "2.0 КБ");
        assert_eq!(format_bytes(3 * 1_048_576), "3.0 МБ");
        assert_eq!(format_bytes(2 * 1_073_741_824), "2.0 ГБ");
    }

    #[test]
    fn default_columns_are_a_subset_of_all() {
        for column in Column::default_visible() {
            assert!(Column::all().contains(&column));
        }
        assert_eq!(Column::default_visible().len(), 7);
    }

    #[test]
    fn every_profile_uses_known_columns() {
        for profile in Profile::all() {
            let columns = profile.columns();
            assert!(!columns.is_empty());
            for column in columns {
                assert!(Column::all().contains(&column), "неизвестная колонка");
            }
        }
    }

    #[test]
    fn basic_profile_matches_the_default_set() {
        assert_eq!(
            Profile::Basic.columns(),
            Column::default_visible().to_vec(),
            "базовый профиль должен совпадать с набором по умолчанию"
        );
    }
}
