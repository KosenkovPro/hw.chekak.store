//! Панель подробностей выбранного процесса.
//!
//! Показано то, чего нет в списке: путь к образу, командная строка, владелец.
//! Эти данные требуют отдельного запроса к процессу, поэтому в горячем пути
//! сэмплера их нет — и в панели они честно помечаются как неизвестные.
//!
//! Отдельная карточка «Данные» показывает, откуда взято значение и насколько
//! ему верить: проект не различает «измерено» и «придумано».

use egui::{Color32, RichText};

use mcpad_core::{ModuleLink, ProcessInfo};
use mcpad_theme::Theme;

use crate::heat::{Heat, rgb_to_color};
use crate::i18n::{Key, Strings};
use crate::widgets;

/// Что показать в панели.
pub enum Details {
    /// Процесс не выбран.
    Empty,
    /// Процесс завершился между снимками.
    Gone,
    /// Данные процесса.
    Process(Box<ProcessInfo>),
}

/// Рисует панель подробностей.
pub fn details_panel(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    details: Details,
    modules: &[ModuleLink],
    modules_error: Option<&str>,
) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);

    match details {
        Details::Empty => {
            ui.add_space(10.0);
            ui.label(
                RichText::new(strings.text(Key::Shortcuts))
                    .size(11.0)
                    .strong()
                    .color(muted),
            );
            ui.add_space(4.0);
            for (keys, action) in [
                ("F6", strings.text(Key::Freeze)),
                ("Ctrl+F", strings.text(Key::Search)),
                ("Ctrl+1…5", strings.text(Key::ViewShortcuts)),
                ("Esc", strings.text(Key::ClearSearch)),
            ] {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(keys)
                            .size(11.0)
                            .monospace()
                            .color(rgb_to_color(theme.palette.accent)),
                    );
                    ui.label(RichText::new(action).size(11.0).color(muted));
                });
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new(strings.text(Key::SelectProcessHint))
                    .size(11.0)
                    .color(muted),
            );
        }
        Details::Gone => {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(strings.text(Key::ProcessGone))
                        .size(12.0)
                        .color(muted),
                );
            });
        }
        Details::Process(process) => {
            let process = process.as_ref();
            ui.add_space(2.0);
            ui.label(RichText::new(&process.name).size(15.0).strong().color(text));
            ui.label(
                RichText::new(format!("PID {}", process.key.pid))
                    .size(11.0)
                    .color(muted),
            );
            ui.add_space(8.0);

            resources_card(ui, theme, strings, process);
            ui.add_space(8.0);
            process_card(ui, theme, strings, process);
            ui.add_space(8.0);
            provenance_card(ui, theme, strings, process);
            ui.add_space(8.0);
            modules_card(ui, theme, strings, modules, modules_error);
        }
    }
}

/// Карточка «Модули»: что загружено в процесс.
///
/// Модули перечисляются по требованию и только для выбранного процесса:
/// перечисление модулей дорого, а список нужен ровно там, где его смотрит
/// пользователь. Если перечисление не удалось — пишем почему.
fn modules_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    modules: &[ModuleLink],
    error: Option<&str>,
) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);

    widgets::section_title(ui, theme, strings.text(Key::Modules));
    shell_card(ui, theme, |ui| {
        ui.set_width(ui.available_width());

        if let Some(error) = error {
            ui.label(RichText::new(error).size(11.0).color(muted));
            return;
        }
        if modules.is_empty() {
            ui.label(
                RichText::new(strings.text(Key::NoData))
                    .size(11.0)
                    .color(muted),
            );
            return;
        }

        // Показываем первые модули: список длинный, а панель узкая.
        const VISIBLE: usize = 12;
        for module in modules.iter().take(VISIBLE) {
            ui.label(
                RichText::new(&module.module_label)
                    .size(11.0)
                    .monospace()
                    .color(text),
            );
        }
        if modules.len() > VISIBLE {
            ui.label(
                RichText::new(format!("… +{}", modules.len() - VISIBLE))
                    .size(10.5)
                    .color(muted),
            );
        }
    });
}

/// Карточка «Ресурсы»: что процесс потребляет прямо сейчас.
fn resources_card(ui: &mut egui::Ui, theme: &Theme, strings: Strings, process: &ProcessInfo) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);

    widgets::section_title(ui, theme, strings.text(Key::Resources));
    shell_card(ui, theme, |ui| {
        ui.set_width(ui.available_width());

        let cpu = process.cpu_percent.value;
        let cpu_heat = Heat::of(cpu.map(|value| value / 100.0));
        let cpu_color = match cpu_heat {
            Heat::None => muted,
            other => other.text(theme),
        };
        row(
            ui,
            strings.text(Key::ColumnCpu),
            &format!("{} %", format_optional(cpu, |value| format!("{value:.1}"))),
            cpu_color,
            text,
            muted,
        );

        let memory = process.memory_working_set.value;
        row(
            ui,
            strings.text(Key::ColumnMemory),
            &format!(
                "{} МБ",
                format_optional(memory, |value| format!("{:.1}", value as f64 / 1_048_576.0))
            ),
            text,
            text,
            muted,
        );

        row(
            ui,
            strings.text(Key::ColumnPrivateMemory),
            &format!(
                "{} МБ",
                format_optional(process.memory_private.value, |value| {
                    format!("{:.1}", value as f64 / 1_048_576.0)
                })
            ),
            text,
            text,
            muted,
        );

        row(
            ui,
            strings.text(Key::ColumnThreads),
            &format_optional(process.thread_count.value, |value| value.to_string()),
            text,
            text,
            muted,
        );

        row(
            ui,
            strings.text(Key::ColumnHandles),
            &format_optional(process.handle_count.value, |value| value.to_string()),
            text,
            text,
            muted,
        );

        row(
            ui,
            strings.text(Key::ColumnIoRead),
            &format_optional(process.io_read_bytes.value, crate::columns::format_bytes),
            text,
            text,
            muted,
        );

        row(
            ui,
            strings.text(Key::ColumnIoWrite),
            &format_optional(process.io_write_bytes.value, crate::columns::format_bytes),
            text,
            text,
            muted,
        );
    });
}

/// Карточка «Процесс»: путь, командная строка, владелец, родитель.
fn process_card(ui: &mut egui::Ui, theme: &Theme, strings: Strings, process: &ProcessInfo) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);

    widgets::section_title(ui, theme, strings.text(Key::ProcessInfo));
    shell_card(ui, theme, |ui| {
        ui.set_width(ui.available_width());

        field(
            ui,
            strings.text(Key::Path),
            process.exe_path.as_deref(),
            text,
            muted,
        );
        field(
            ui,
            strings.text(Key::CommandLine),
            process.command_line.as_deref(),
            text,
            muted,
        );
        field(
            ui,
            strings.text(Key::ColumnUser),
            process.user.as_deref(),
            text,
            muted,
        );
        field(
            ui,
            strings.text(Key::Parent),
            process.parent_pid.map(|pid| pid.to_string()).as_deref(),
            text,
            muted,
        );

        match process.is_protected {
            Some(true) => {
                ui.label(
                    RichText::new(strings.text(Key::Protected))
                        .size(11.0)
                        .color(rgb_to_color(theme.palette.warning)),
                );
            }
            Some(false) => {
                ui.label(
                    RichText::new(strings.text(Key::NotProtected))
                        .size(11.0)
                        .color(muted),
                );
            }
            None => {
                ui.label(
                    RichText::new(strings.text(Key::ProtectionUnknown))
                        .size(11.0)
                        .color(muted),
                );
            }
        }
    });
}

/// Карточка «Данные»: откуда значение и насколько ему верить.
fn provenance_card(ui: &mut egui::Ui, theme: &Theme, strings: Strings, process: &ProcessInfo) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);

    widgets::section_title(ui, theme, strings.text(Key::DataProvenance));
    shell_card(ui, theme, |ui| {
        ui.set_width(ui.available_width());

        let source = process
            .cpu_percent
            .source
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| strings.text(Key::Unknown).to_string());
        row(ui, strings.text(Key::Source), &source, text, text, muted);

        let confidence = process
            .cpu_percent
            .confidence
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| strings.text(Key::Unknown).to_string());
        row(
            ui,
            strings.text(Key::Confidence),
            &confidence,
            text,
            text,
            muted,
        );

        if let Some(reason) = process.cpu_percent.unavailable_reason {
            ui.label(
                RichText::new(format!(
                    "{}: {}",
                    strings.text(Key::Unknown),
                    reason.as_str()
                ))
                .size(11.0)
                .color(rgb_to_color(theme.palette.warning)),
            );
        }
    });
}

/// Строка «подпись — значение».
fn row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    value_color: Color32,
    text: Color32,
    muted: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.5).color(muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
            ui.label(RichText::new(value).size(11.5).color(value_color));
        });
    });
    let _ = text;
}

/// Поле со значением; `None` — «неизвестно», а не пустота.
fn field(ui: &mut egui::Ui, label: &str, value: Option<&str>, text: Color32, muted: Color32) {
    ui.label(RichText::new(label).size(11.0).color(muted));
    let shown = value.unwrap_or("—");
    let color = if value.is_some() { text } else { muted };
    ui.label(RichText::new(shown).size(11.5).color(color).monospace());
    ui.add_space(4.0);
}

/// Карточка с рамкой.
fn shell_card(ui: &mut egui::Ui, theme: &Theme, add_contents: impl FnOnce(&mut egui::Ui)) {
    crate::shell::card(ui, theme, add_contents);
}

/// Форматирует значение или ставит прочерк.
fn format_optional<T>(value: Option<T>, format: impl FnOnce(T) -> String) -> String {
    match value {
        Some(value) => format(value),
        None => "—".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{Counter, ProcessKey};

    #[test]
    fn missing_value_is_a_dash() {
        assert_eq!(format_optional(None::<u32>, |value| value.to_string()), "—");
        assert_eq!(format_optional(Some(5u32), |value| value.to_string()), "5");
    }

    #[test]
    fn protection_state_has_three_outcomes() {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(1), "a.exe");
        assert_eq!(process.is_protected, None);
        process.is_protected = Some(true);
        assert_eq!(process.is_protected, Some(true));
    }

    #[test]
    fn counter_without_value_has_no_source() {
        let counter: Counter<u32> =
            Counter::unavailable(mcpad_core::UnavailableReason::NotMeasured);
        assert!(counter.source.is_none());
        assert!(counter.value.is_none());
    }
}
