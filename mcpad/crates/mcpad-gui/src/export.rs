//! Экспорт видимой таблицы процессов.
//!
//! Окно ничего не пишет на диск: экспорт уходит в буфер обмена, а сохранить
//! файл пользователь решает сам. Так приложение не оставляет мусор там, куда
//! его не звали.
//!
//! Формат тот же, что и у CLI: те же колонки, те же значения. Неизвестное
//! значение остаётся неизвестным — в CSV это пустая ячейка, в JSON `null`.

use mcpad_core::ProcessInfo;

use crate::columns::Column;
use crate::i18n::{Key, Strings};

/// Формат вывода.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Значения, разделённые запятыми.
    Csv,
    /// Массив объектов.
    Json,
}

/// Колонки, которые имеет смысл выгружать: график истории — не значение.
fn exportable(columns: &[Column]) -> Vec<Column> {
    columns
        .iter()
        .copied()
        .filter(|column| *column != Column::History)
        .collect()
}

/// Заголовок колонки в выбранном языке.
fn heading(column: Column, strings: Strings) -> &'static str {
    column.title(strings)
}

/// Значение ячейки в виде текста; `None` — значение неизвестно.
fn cell(column: Column, process: &ProcessInfo) -> Option<String> {
    column.cell(process)
}

/// Выгружает таблицу в CSV.
pub fn to_csv(columns: &[Column], rows: &[&ProcessInfo], strings: Strings) -> String {
    let columns = exportable(columns);
    let mut out = String::new();

    let headings: Vec<&str> = columns
        .iter()
        .map(|column| heading(*column, strings))
        .collect();
    out.push_str(
        &headings
            .iter()
            .map(|value| quote(value))
            .collect::<Vec<String>>()
            .join(","),
    );
    out.push('\n');

    for process in rows {
        let values: Vec<String> = columns
            .iter()
            .map(|column| match cell(*column, process) {
                Some(value) => quote(&value),
                None => String::new(),
            })
            .collect();
        out.push_str(&values.join(","));
        out.push('\n');
    }
    out
}

/// Выгружает таблицу в JSON.
///
/// Числа остаются числами: файл должен брать машина, а не человек.
pub fn to_json(columns: &[Column], rows: &[&ProcessInfo], strings: Strings) -> String {
    let columns = exportable(columns);
    let mut out = String::from("[\n");

    for (index, process) in rows.iter().enumerate() {
        let fields: Vec<String> = columns
            .iter()
            .map(|column| {
                let key = json_key(*column);
                let value = json_value(*column, process);
                format!("    \"{key}\": {value}")
            })
            .collect();
        out.push_str("  {\n");
        out.push_str(&fields.join(",\n"));
        out.push_str("\n  }");
        if index + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("]\n");
    let _ = strings;
    out
}

/// Значение поля в JSON.
fn json_value(column: Column, process: &ProcessInfo) -> String {
    let number = |value: Option<f64>| match value {
        Some(value) => format!("{value:.4}"),
        None => "null".to_string(),
    };

    match column {
        Column::Pid => process.key.pid.to_string(),
        Column::Name => json_string(&process.name),
        Column::Cpu => number(process.cpu_percent.value),
        Column::Memory => number(
            process
                .memory_working_set
                .value
                .map(|value| value as f64 / 1_048_576.0),
        ),
        Column::PrivateMemory => number(
            process
                .memory_private
                .value
                .map(|value| value as f64 / 1_048_576.0),
        ),
        Column::Threads => number(process.thread_count.value.map(f64::from)),
        Column::Handles => number(process.handle_count.value.map(f64::from)),
        // Объёмы I/O остаются текстом: единица выбирается по величине.
        Column::IoRead => match process.io_read_bytes.value {
            Some(value) => json_string(&crate::columns::format_bytes(value)),
            None => "null".to_string(),
        },
        Column::IoWrite => match process.io_write_bytes.value {
            Some(value) => json_string(&crate::columns::format_bytes(value)),
            None => "null".to_string(),
        },
        Column::History => "null".to_string(),
    }
}

/// Ключ поля в JSON.
fn json_key(column: Column) -> &'static str {
    match column {
        Column::Pid => "pid",
        Column::Name => "name",
        Column::Cpu => "cpu_percent",
        Column::Memory => "memory_working_set_mb",
        Column::PrivateMemory => "memory_private_mb",
        Column::Threads => "threads",
        Column::Handles => "handles",
        Column::IoRead => "io_read",
        Column::IoWrite => "io_write",
        Column::History => "history",
    }
}

/// Экранирует строку для JSON.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

/// Оборачивает значение в кавычки, если в нём есть запятая или кавычка.
fn quote(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Подсказка формата для панели: что попадёт в буфер.
pub fn format_label(format: Format, strings: Strings) -> &'static str {
    match format {
        Format::Csv => strings.text(Key::ExportCsv),
        Format::Json => strings.text(Key::ExportJson),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey, Source};

    fn process(pid: u32, name: &str, cpu: Option<f64>) -> ProcessInfo {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(pid), name);
        if let Some(value) = cpu {
            process.cpu_percent = Counter::measured(value, Source::Derived);
        }
        process
    }

    fn rows(processes: &[ProcessInfo]) -> Vec<&ProcessInfo> {
        processes.iter().collect()
    }

    #[test]
    fn csv_has_header_and_rows() {
        let processes = vec![
            process(1, "a.exe", Some(10.0)),
            process(2, "b.exe", Some(5.0)),
        ];
        let columns = [Column::Pid, Column::Name, Column::Cpu];
        let csv = to_csv(&columns, &rows(&processes), Strings::default());
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("PID"));
        assert!(lines[1].contains("a.exe"));
        assert!(lines[1].contains("10.0"));
    }

    #[test]
    fn csv_quotes_values_with_commas() {
        let processes = vec![process(1, "we,ird.exe", Some(1.0))];
        let columns = [Column::Name];
        let csv = to_csv(&columns, &rows(&processes), Strings::default());
        assert!(csv.contains("\"we,ird.exe\""));
    }

    #[test]
    fn json_keeps_unknown_values_null() {
        let processes = vec![process(1, "a.exe", None)];
        let columns = [Column::Pid, Column::Name, Column::Cpu];
        let json = to_json(&columns, &rows(&processes), Strings::default());
        assert!(json.contains("\"pid\": 1"));
        assert!(json.contains("\"cpu_percent\": null"));
    }

    #[test]
    fn history_column_is_not_exported() {
        let processes = vec![process(1, "a.exe", Some(1.0))];
        let columns = [Column::Pid, Column::Name, Column::History];
        let csv = to_csv(&columns, &rows(&processes), Strings::default());
        assert!(!csv.to_lowercase().contains("история"));
        assert!(!csv.to_lowercase().contains("history"));
    }
}
