//! Каркас окна: боковая навигация, командная панель, строка состояния.
//!
//! Раскладка повторяет Диспетчер задач Windows 11: слева — сворачиваемая
//! навигация с подписями, сверху — командная панель с поиском, снизу — строка
//! состояния. В отличие от системного диспетчера, здесь те же приёмы применены
//! к единому сэмплеру: цифры в окне и в командной строке совпадают.

use egui::{Color32, RichText, Sense, Stroke, Vec2};

use mcpad_theme::Theme;

use crate::heat::rgb_to_color;
use crate::i18n::{Key, Strings};
use crate::widgets::{self, NavIcon};

/// Ширина развёрнутой боковой навигации.
pub const SIDEBAR_WIDTH: f32 = 208.0;
/// Ширина свёрнутой боковой навигации.
pub const SIDEBAR_COLLAPSED: f32 = 56.0;

/// Представление окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Список процессов.
    Processes,
    /// Дерево процессов.
    Tree,
    /// Производительность.
    Performance,
    /// Canvas-граф.
    Canvas,
    /// Возможности платформы.
    Capabilities,
}

impl View {
    /// Все представления в порядке навигации.
    pub fn all() -> [View; 5] {
        [
            View::Processes,
            View::Tree,
            View::Performance,
            View::Canvas,
            View::Capabilities,
        ]
    }

    /// Значок представления.
    pub fn icon(self) -> NavIcon {
        match self {
            View::Processes => NavIcon::Processes,
            View::Tree => NavIcon::Tree,
            View::Performance => NavIcon::Performance,
            View::Canvas => NavIcon::Canvas,
            View::Capabilities => NavIcon::Capabilities,
        }
    }

    /// Подпись представления.
    pub fn title(self, strings: Strings) -> &'static str {
        match self {
            View::Processes => strings.text(Key::ViewProcesses),
            View::Tree => strings.text(Key::ViewTree),
            View::Performance => strings.text(Key::ViewPerformance),
            View::Canvas => strings.text(Key::ViewCanvas),
            View::Capabilities => strings.text(Key::ViewCapabilities),
        }
    }

    /// Представление по коду; неизвестный код — `None`.
    pub fn from_code(code: &str) -> Option<View> {
        match code.trim().to_ascii_lowercase().as_str() {
            "processes" | "process" => Some(View::Processes),
            "tree" => Some(View::Tree),
            "performance" => Some(View::Performance),
            "canvas" => Some(View::Canvas),
            "capabilities" => Some(View::Capabilities),
            _ => None,
        }
    }

    /// Код представления для командной строки.
    pub fn code(self) -> &'static str {
        match self {
            View::Processes => "processes",
            View::Tree => "tree",
            View::Performance => "performance",
            View::Canvas => "canvas",
            View::Capabilities => "capabilities",
        }
    }
}

/// Состояние боковой навигации.
#[derive(Debug, Clone, Copy)]
pub struct NavState {
    /// Выбранное представление.
    pub view: View,
    /// Свёрнута ли навигация до одних значков.
    pub collapsed: bool,
}

/// Команды, собранные командной панелью.
#[derive(Debug, Clone, Copy, Default)]
pub struct Commands {
    /// Заморозить или размораживать данные.
    pub toggle_freeze: bool,
    /// Переключить тёмную и светлую тему.
    pub toggle_theme: bool,
    /// Переключить язык.
    pub toggle_language: bool,
    /// Показать или скрыть панель подробностей.
    pub toggle_details: bool,
    /// Перечитать файл темы.
    pub reload_theme: bool,
}

/// Боковая навигация.
pub fn sidebar(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    state: &mut NavState,
    subtitle: &str,
) {
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);
    let accent = rgb_to_color(theme.palette.accent);
    let collapsed = state.collapsed;

    ui.add_space(6.0);
    ui.label(RichText::new("MCPad").size(17.0).strong().color(text));
    if !collapsed {
        ui.label(RichText::new(subtitle).size(11.0).color(muted));
    }
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    for view in View::all() {
        let selected = state.view == view;
        let fill = if selected {
            accent.gamma_multiply(0.28)
        } else {
            Color32::TRANSPARENT
        };
        let label_color = if selected { text } else { muted };
        let icon_color = if selected { accent } else { muted };

        let response = egui::Frame::new()
            .fill(fill)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(8, 7))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                    widgets::nav_icon(ui.painter(), rect, view.icon(), icon_color);
                    if !collapsed {
                        ui.label(
                            RichText::new(view.title(strings))
                                .size(13.0)
                                .color(label_color),
                        );
                    }
                });
            })
            .response;

        if response.clicked() {
            state.view = view;
        }
        if response.hovered() {
            response.on_hover_text(view.title(strings));
        }
    }

    // Свёрнутая навигация прикреплена к низу: так кнопка всегда под рукой.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        let label = if collapsed { "»" } else { "«" };
        let response = ui.add_sized([SIDEBAR_COLLAPSED - 16.0, 26.0], egui::Button::new(label));
        if response.clicked() {
            state.collapsed = !state.collapsed;
        }
        ui.separator();
        ui.add_space(2.0);
    });
}

/// Командная панель: поиск, заморозка, сведения, тема, язык.
#[allow(clippy::too_many_arguments)]
pub fn command_bar(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    filter: &mut String,
    filter_mode: &mut crate::filter::Mode,
    frozen: bool,
    details_visible: bool,
    has_theme_file: bool,
    focus_search: &mut bool,
    commands: &mut Commands,
) {
    let surface = rgb_to_color(theme.palette.surface);
    let text = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);
    let accent = rgb_to_color(theme.palette.accent);

    ui.horizontal(|ui| {
        ui.add_space(4.0);

        // Поиск: подсказка внутри поля, а не отдельной подписью.
        let response = ui.add(
            egui::TextEdit::singleline(filter)
                .hint_text(RichText::new(strings.text(Key::SearchHint)).color(muted))
                .desired_width(240.0),
        );
        if *focus_search {
            response.request_focus();
            *focus_search = false;
        }
        if filter.is_empty() {
            response.on_hover_text(strings.text(Key::Search));
        }

        // Режим поиска: обычный текст или регулярное выражение. В Task
        // Explorer и System Informer это переключатель рядом с полем.
        egui::ComboBox::from_id_salt("filter_mode")
            .selected_text(RichText::new(filter_mode_label(strings, *filter_mode)).size(12.0))
            .width(150.0)
            .show_ui(ui, |ui| {
                for mode in crate::filter::Mode::all() {
                    ui.selectable_value(filter_mode, mode, filter_mode_label(strings, mode));
                }
            });

        ui.add_space(6.0);

        // Заморозка данных.
        let freeze_label = if frozen {
            strings.text(Key::FrozenShort)
        } else {
            strings.text(Key::FreezeShort)
        };
        let freeze = ui
            .add(
                egui::Button::new(RichText::new(freeze_label).size(12.0).color(if frozen {
                    accent
                } else {
                    text
                }))
                .fill(if frozen {
                    accent.gamma_multiply(0.28)
                } else {
                    surface
                }),
            )
            .on_hover_text(strings.text(Key::Freeze));
        if freeze.clicked() {
            commands.toggle_freeze = true;
        }

        // Панель подробностей.
        let details = ui
            .add(
                egui::Button::new(
                    RichText::new(strings.text(Key::Details))
                        .size(12.0)
                        .color(if details_visible { accent } else { text }),
                )
                .fill(if details_visible {
                    accent.gamma_multiply(0.28)
                } else {
                    surface
                }),
            )
            .on_hover_text(strings.text(Key::DetailsPanel));
        if details.clicked() {
            commands.toggle_details = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
            let language = ui
                .add(
                    egui::Button::new(
                        RichText::new(strings.lang.self_name())
                            .size(12.0)
                            .color(text),
                    )
                    .fill(surface),
                )
                .on_hover_text(strings.text(Key::Language));
            if language.clicked() {
                commands.toggle_language = true;
            }

            let theme_label = if theme.dark {
                strings.text(Key::Light)
            } else {
                strings.text(Key::Dark)
            };
            let theme_button = ui
                .add(
                    egui::Button::new(RichText::new(theme_label).size(12.0).color(text))
                        .fill(surface),
                )
                .on_hover_text(strings.text(Key::Theme));
            if theme_button.clicked() {
                commands.toggle_theme = true;
            }

            // Перечитывание файла темы — только если он задан.
            if has_theme_file {
                let reload = ui
                    .add(
                        egui::Button::new(
                            RichText::new(strings.text(Key::ReloadTheme))
                                .size(12.0)
                                .color(text),
                        )
                        .fill(surface),
                    )
                    .on_hover_text(strings.text(Key::ReloadTheme));
                if reload.clicked() {
                    commands.reload_theme = true;
                }
            }
        });
    });
}

/// Строка состояния: что снято, когда и с каким интервалом.
#[allow(clippy::too_many_arguments)]
pub fn status_bar(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    snapshot_time: Option<String>,
    process_count: Option<usize>,
    interval: &mut std::time::Duration,
    frozen: bool,
    platform: &str,
) {
    let muted = rgb_to_color(theme.palette.muted);
    let text = rgb_to_color(theme.palette.text);
    let accent = rgb_to_color(theme.palette.accent);

    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if frozen {
            ui.label(
                RichText::new(strings.text(Key::Frozen))
                    .size(11.0)
                    .strong()
                    .color(accent),
            );
            ui.separator();
        }
        if let Some(time) = snapshot_time {
            ui.label(
                RichText::new(format!("{}: {time}", strings.text(Key::Snapshot)))
                    .size(11.0)
                    .color(muted),
            );
            ui.separator();
        }
        if let Some(count) = process_count {
            ui.label(
                RichText::new(format!("{}: {count}", strings.text(Key::Total)))
                    .size(11.0)
                    .color(muted),
            );
            ui.separator();
        }

        // Интервал обновления выбирается прямо здесь: в System Informer это
        // View → Refresh interval, в Task Explorer — частота опроса.
        ui.label(
            RichText::new(format!("{}:", strings.text(Key::Interval)))
                .size(11.0)
                .color(muted),
        );
        egui::ComboBox::from_id_salt("refresh_interval")
            .selected_text(RichText::new(interval_label(*interval, strings)).size(11.0))
            .width(78.0)
            .show_ui(ui, |ui| {
                for option in INTERVALS {
                    if ui
                        .selectable_label(*interval == option, interval_label(option, strings))
                        .clicked()
                    {
                        *interval = option;
                    }
                }
            });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
            if !platform.is_empty() {
                widgets::chip(ui, theme, platform, muted);
            }
            ui.label(RichText::new("MCPad").size(11.0).color(text));
        });
    });
}

/// Варианты интервала обновления.
pub const INTERVALS: [std::time::Duration; 5] = [
    std::time::Duration::from_millis(500),
    std::time::Duration::from_millis(1000),
    std::time::Duration::from_millis(2000),
    std::time::Duration::from_millis(5000),
    std::time::Duration::from_millis(10000),
];

/// Подпись интервала: ровные секунды — секундами, остальное — миллисекундами.
pub fn interval_label(interval: std::time::Duration, strings: Strings) -> String {
    let millis = interval.as_millis();
    if millis >= 1000 && millis.is_multiple_of(1000) {
        format!("{} {}", millis / 1000, strings.text(Key::Seconds))
    } else {
        format!("{millis} {}", strings.text(Key::Milliseconds))
    }
}

/// Подпись режима поиска.
pub fn filter_mode_label(strings: Strings, mode: crate::filter::Mode) -> &'static str {
    match mode {
        crate::filter::Mode::Text => strings.text(Key::FilterModeText),
        crate::filter::Mode::Regex => strings.text(Key::FilterModeRegex),
    }
}

/// Рамка-карточка вокруг содержимого.
pub fn card(ui: &mut egui::Ui, theme: &Theme, add_contents: impl FnOnce(&mut egui::Ui)) {
    let surface = rgb_to_color(theme.palette.surface);
    let edge = rgb_to_color(theme.palette.edge);
    egui::Frame::new()
        .fill(surface)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(widgets::RADIUS)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, add_contents);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_have_unique_titles() {
        let strings = Strings::new(crate::i18n::Lang::Ru);
        let mut titles: Vec<&str> = View::all().iter().map(|view| view.title(strings)).collect();
        titles.sort_unstable();
        let count = titles.len();
        titles.dedup();
        assert_eq!(titles.len(), count);
    }

    #[test]
    fn sidebar_is_wider_when_expanded() {
        let widths = [SIDEBAR_WIDTH, SIDEBAR_COLLAPSED];
        assert!(widths[0] > widths[1]);
    }
}
