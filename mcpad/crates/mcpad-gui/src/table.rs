//! Таблица процессов: тепловая карта, сортировка по заголовкам, мини-графики.
//!
//! Таблица рисуется вручную, а не через `egui::Grid`: у каждой ячейки своя
//! ширина и своя подложка, а выделение строки должно тянуться от края до края.
//! `Grid` для этого не годится.
//!
//! Совпадает с Диспетчером задач Windows 11: цветом в колонке показывается
//! нагрузка, заголовок переключает сортировку, справа стоит мини-график доли
//! CPU за последние секунды.

use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Vec2};

use mcpad_core::ProcessInfo;
use mcpad_theme::Theme;

use crate::columns::{Column, SortValue};
use crate::heat::{Heat, rgb_to_color};
use crate::i18n::{Key, Strings};
use crate::widgets;

/// Высота строки в обычном режиме.
pub const ROW_HEIGHT: f32 = 22.0;
/// Высота строки в компактном режиме.
pub const ROW_HEIGHT_COMPACT: f32 = 17.0;
/// Высота заголовка таблицы.
pub const HEADER_HEIGHT: f32 = 26.0;
/// Отступ между колонками.
pub const COLUMN_GAP: f32 = 10.0;
/// Сколько последних отсчётов показывает мини-график.
pub const SPARK_SAMPLES: usize = 60;

/// Состояние таблицы.
#[derive(Debug, Clone)]
pub struct TableState {
    /// Колонка сортировки.
    pub sort: Column,
    /// Сортировка по убыванию.
    pub descending: bool,
    /// Показываемые колонки.
    pub visible: Vec<Column>,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            sort: Column::Cpu,
            descending: true,
            visible: Column::default_visible().to_vec(),
        }
    }
}

impl TableState {
    /// Переключает сортировку по колонке.
    ///
    /// Повторный щелчок по той же колонке меняет направление.
    pub fn toggle_sort(&mut self, column: Column) {
        if self.sort == column {
            self.descending = !self.descending;
        } else {
            self.sort = column;
            // По имени всегда по возрастанию: иначе список выглядит случайным.
            self.descending = column.numeric();
        }
    }

    /// Показывается ли колонка.
    pub fn shows(&self, column: Column) -> bool {
        self.visible.contains(&column)
    }

    /// Включает или выключает колонку.
    pub fn set_visible(&mut self, column: Column, visible: bool) {
        if visible {
            if !self.visible.contains(&column) {
                self.visible.push(column);
                self.visible.sort_by_key(|item| {
                    Column::all()
                        .iter()
                        .position(|candidate| candidate == item)
                        .unwrap_or(usize::MAX)
                });
            }
        } else {
            self.visible.retain(|item| *item != column);
        }
    }
}

/// Раскладка колонок: ширины и общая ширина таблицы.
pub struct Layout {
    /// Ширина каждой показываемой колонки.
    pub widths: Vec<f32>,
    /// Общая ширина таблицы.
    pub total_width: f32,
}

impl Layout {
    /// Раскладка для набора колонок.
    pub fn new(columns: &[Column]) -> Self {
        let widths: Vec<f32> = columns.iter().map(|column| column.width()).collect();
        let gaps = COLUMN_GAP * columns.len().saturating_sub(1) as f32;
        let total_width = widths.iter().sum::<f32>() + gaps;
        Self {
            widths,
            total_width,
        }
    }

    /// Смещение левого края колонки от начала таблицы.
    ///
    /// Нужно, чтобы итоги группы встали ровно в те же колонки, что и строки
    /// процессов: иначе числа разъезжаются.
    pub fn column_offset(&self, columns: &[Column], target: Column) -> Option<f32> {
        let mut offset = 0.0;
        for (index, column) in columns.iter().enumerate() {
            if *column == target {
                return Some(offset);
            }
            offset += self.widths.get(index).copied().unwrap_or(0.0) + COLUMN_GAP;
        }
        None
    }

    /// Ширина колонки по её названию.
    pub fn column_width(&self, columns: &[Column], target: Column) -> Option<f32> {
        columns
            .iter()
            .position(|column| *column == target)
            .and_then(|index| self.widths.get(index).copied())
    }
}

/// Строка таблицы: процесс или заголовок группы.
#[derive(Debug, Clone)]
pub enum TableRow<'a> {
    /// Заголовок группы с итогами.
    Group(GroupHeader),
    /// Процесс.
    Process(&'a ProcessInfo),
}

/// Итоги группы в заголовке.
#[derive(Debug, Clone)]
pub struct GroupHeader {
    /// Название группы.
    pub label: String,
    /// Число процессов в группе.
    pub count: usize,
    /// Суммарная доля CPU; `None` — неизвестно.
    pub cpu: Option<f64>,
    /// Суммарный рабочий набор, байты; `None` — неизвестно.
    pub memory: Option<u64>,
}

/// Сортирует строки по выбранной колонке.
///
/// Процессы без значения уходят в конец при любом направлении.
pub fn sort_rows(rows: &mut [&ProcessInfo], state: &TableState) {
    let column = state.sort;
    let descending = state.descending;
    rows.sort_by(|left, right| {
        let left_value = column.value(left);
        let right_value = column.value(right);
        // Направление применяется только к сравнению самих значений: процессы
        // без данных обязаны остаться в конце при любой сортировке.
        match (left_value, right_value) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left), Some(right)) => {
                let ordering = SortValue::compare_values(&left, &right);
                if descending {
                    ordering.reverse()
                } else {
                    ordering
                }
            }
        }
    });
}

/// Рисует заголовок таблицы: щёлчок по названию переключает сортировку.
pub fn header(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    state: &mut TableState,
    layout: &Layout,
) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(layout.total_width, HEADER_HEIGHT), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);

    let muted = rgb_to_color(theme.palette.muted);
    let accent = rgb_to_color(theme.palette.accent);
    let grid = rgb_to_color(theme.palette.grid);

    let table_id = ui.scope_id().with("process_header");
    // Колонки копируются: внутри цикла состояние меняется по клику.
    let columns = state.visible.clone();
    let mut x = rect.min.x;
    for (index, column) in columns.iter().enumerate() {
        let Some(width) = layout.widths.get(index).copied() else {
            break;
        };
        let cell = Rect::from_min_size(egui::pos2(x, rect.min.y), Vec2::new(width, HEADER_HEIGHT));
        let response = ui.interact(cell, table_id.with(index), Sense::click());

        let active = state.sort == *column;
        let color = if active { accent } else { muted };
        let title = column.title(strings);
        let mut label = String::from(title);
        if active {
            label.push_str(if state.descending { " ↓" } else { " ↑" });
        }

        if response.hovered() {
            painter.rect_filled(cell, CornerRadius::from(4.0), accent.gamma_multiply(0.12));
        }
        painter.text(
            cell.left_center() + Vec2::new(4.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            color,
        );
        painter.line_segment(
            [
                egui::pos2(cell.min.x, cell.max.y),
                egui::pos2(cell.max.x, cell.max.y),
            ],
            Stroke::new(1.0, grid),
        );

        if response.clicked() {
            state.toggle_sort(*column);
        }
        if response.hovered() {
            response.on_hover_text(title);
        }
        x += width + COLUMN_GAP;
    }
}

/// Рисует строки таблицы.
///
/// Возвращает PID, если пользователь щёлкнул по строке.
#[allow(clippy::too_many_arguments)]
pub fn rows(
    ui: &mut egui::Ui,
    theme: &Theme,
    strings: Strings,
    columns: &[Column],
    layout: &Layout,
    rows: &[TableRow<'_>],
    memory_total: Option<u64>,
    sparklines: &HashMap<u32, Vec<Option<f64>>>,
    selected: Option<u32>,
    compact: bool,
) -> Option<u32> {
    let row_height = if compact {
        ROW_HEIGHT_COMPACT
    } else {
        ROW_HEIGHT
    };
    let height = row_height * rows.len() as f32;

    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(layout.total_width, height.max(row_height)),
        Sense::hover(),
    );
    let painter = ui.painter().with_clip_rect(rect);

    let text_color = rgb_to_color(theme.palette.text);
    let muted = rgb_to_color(theme.palette.muted);
    let accent = rgb_to_color(theme.palette.accent);
    let surface = rgb_to_color(theme.palette.surface);
    let grid = rgb_to_color(theme.palette.grid);

    let table_id = ui.scope_id().with("process_rows");
    let mut picked: Option<u32> = None;

    for (row_index, row) in rows.iter().enumerate() {
        let top = rect.min.y + row_height * row_index as f32;
        let row_rect = Rect::from_min_size(
            egui::pos2(rect.min.x, top),
            Vec2::new(layout.total_width, row_height),
        );

        let process = match row {
            TableRow::Group(header) => {
                draw_group_header(&painter, row_rect, header, columns, layout, theme);
                continue;
            }
            TableRow::Process(process) => *process,
        };

        let response = ui.interact(row_rect, table_id.with(process.key.pid), Sense::click());

        let is_selected = selected == Some(process.key.pid);
        if is_selected {
            painter.rect_filled(
                row_rect,
                CornerRadius::from(4.0),
                accent.gamma_multiply(0.20),
            );
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(row_rect.min.x, row_rect.min.y + 2.0),
                    Vec2::new(2.0, row_rect.height() - 4.0),
                ),
                CornerRadius::from(1.0),
                accent,
            );
        } else if response.hovered() {
            painter.rect_filled(row_rect, CornerRadius::from(4.0), surface);
        }
        if response.clicked() {
            picked = Some(process.key.pid);
        }

        let mut cell_x = rect.min.x;
        for (index, column) in columns.iter().enumerate() {
            let Some(width) = layout.widths.get(index).copied() else {
                break;
            };
            let cell = Rect::from_min_size(egui::pos2(cell_x, top), Vec2::new(width, row_height));

            if *column == Column::History {
                let empty = Vec::new();
                let samples = sparklines.get(&process.key.pid).unwrap_or(&empty);
                draw_sparkline(&painter, cell, samples, accent, grid);
            } else {
                let heat = Heat::of(column.heat_ratio(process, memory_total));
                if heat != Heat::None {
                    painter.rect_filled(
                        Rect::from_min_size(
                            cell.min + Vec2::new(1.0, 2.0),
                            Vec2::new(cell.width() - 2.0, cell.height() - 4.0),
                        ),
                        CornerRadius::from(4.0),
                        heat.fill(theme),
                    );
                }
                let content = column.cell(process);
                let label = content.clone().unwrap_or_else(|| "—".to_string());
                let color = match (&content, heat) {
                    (None, _) => muted,
                    (Some(_), Heat::None) => text_color,
                    (Some(_), other) => other.text(theme),
                };
                let font = if column.numeric() {
                    FontId::monospace(11.5)
                } else {
                    FontId::proportional(12.0)
                };
                let (anchor, position) = match column.align() {
                    egui::Align::LEFT => (
                        Align2::LEFT_CENTER,
                        cell.left_center() + Vec2::new(4.0, 0.0),
                    ),
                    _ => (
                        Align2::RIGHT_CENTER,
                        cell.right_center() + Vec2::new(-4.0, 0.0),
                    ),
                };
                painter.text(position, anchor, label, font, color);
            }

            cell_x += width + COLUMN_GAP;
        }
    }

    if rows.is_empty() {
        painter.text(
            rect.left_top() + Vec2::new(4.0, 6.0),
            Align2::LEFT_TOP,
            strings.text(Key::NoData),
            FontId::proportional(12.0),
            muted,
        );
    }

    picked
}

/// Рисует заголовок группы: имя и итоги в тех же колонках, что и строки.
fn draw_group_header(
    painter: &egui::Painter,
    row_rect: Rect,
    header: &GroupHeader,
    columns: &[Column],
    layout: &Layout,
    theme: &Theme,
) {
    let accent = rgb_to_color(theme.palette.accent);
    let muted = rgb_to_color(theme.palette.muted);
    let surface = rgb_to_color(theme.palette.surface);
    let grid = rgb_to_color(theme.palette.grid);

    painter.rect_filled(row_rect, CornerRadius::from(4.0), surface);
    painter.line_segment(
        [
            egui::pos2(row_rect.min.x, row_rect.max.y),
            egui::pos2(row_rect.max.x, row_rect.max.y),
        ],
        Stroke::new(1.0, grid),
    );

    painter.text(
        row_rect.left_center() + Vec2::new(6.0, 0.0),
        Align2::LEFT_CENTER,
        format!("{}  ({})", header.label, header.count),
        FontId::proportional(12.0),
        accent,
    );

    let cpu = match header.cpu {
        Some(value) => format!("{value:.1}"),
        None => "—".to_string(),
    };
    draw_group_total(painter, row_rect, columns, layout, Column::Cpu, cpu, muted);

    let memory = match header.memory {
        Some(value) => format!("{:.1}", value as f64 / 1_048_576.0),
        None => "—".to_string(),
    };
    draw_group_total(
        painter,
        row_rect,
        columns,
        layout,
        Column::Memory,
        memory,
        muted,
    );
}

/// Рисует итог группы в правом краю своей колонки.
fn draw_group_total(
    painter: &egui::Painter,
    row_rect: Rect,
    columns: &[Column],
    layout: &Layout,
    column: Column,
    text: String,
    color: Color32,
) {
    let Some(offset) = layout.column_offset(columns, column) else {
        return;
    };
    let Some(width) = layout.column_width(columns, column) else {
        return;
    };
    painter.text(
        egui::pos2(row_rect.min.x + offset + width - 4.0, row_rect.center().y),
        Align2::RIGHT_CENTER,
        text,
        FontId::monospace(11.5),
        color,
    );
}

/// Рисует мини-график доли CPU.
fn draw_sparkline(
    painter: &egui::Painter,
    cell: Rect,
    samples: &[Option<f64>],
    color: Color32,
    grid: Color32,
) {
    let inner = Rect::from_min_size(
        cell.min + Vec2::new(2.0, 4.0),
        Vec2::new(cell.width() - 4.0, cell.height() - 8.0),
    );
    widgets::sparkline_in_rect(painter, inner, samples, color, grid);
}

/// Кнопка выбора столбцов.
pub fn column_chooser(ui: &mut egui::Ui, strings: Strings, state: &mut TableState) {
    egui::ComboBox::from_id_salt("columns")
        .selected_text(RichText::new(strings.text(Key::Columns)).size(12.0))
        .show_ui(ui, |ui| {
            ui.label(RichText::new(strings.text(Key::Columns)).size(11.0));
            ui.separator();
            for column in Column::all() {
                let mut visible = state.shows(column);
                if ui
                    .checkbox(
                        &mut visible,
                        RichText::new(column.title(strings)).size(12.0),
                    )
                    .changed()
                {
                    state.set_visible(column, visible);
                }
            }
        });
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

    #[test]
    fn sorting_by_cpu_descending_puts_busiest_first() {
        let state = TableState {
            sort: Column::Cpu,
            descending: true,
            ..TableState::default()
        };
        let processes = [
            process(1, "a", Some(1.0)),
            process(2, "b", Some(9.0)),
            process(3, "c", Some(5.0)),
        ];
        let mut rows: Vec<&ProcessInfo> = processes.iter().collect();
        sort_rows(&mut rows, &state);
        assert_eq!(rows[0].key.pid, 2);
        assert_eq!(rows[2].key.pid, 1);
    }

    #[test]
    fn unknown_values_sink_to_the_bottom() {
        let state = TableState {
            sort: Column::Cpu,
            descending: true,
            ..TableState::default()
        };
        let processes = [
            process(1, "a", None),
            process(2, "b", Some(9.0)),
            process(3, "c", None),
        ];
        let mut rows: Vec<&ProcessInfo> = processes.iter().collect();
        sort_rows(&mut rows, &state);
        assert_eq!(rows[0].key.pid, 2);
        assert!(rows[1].key.pid == 1 || rows[1].key.pid == 3);
        assert!(rows[2].key.pid == 1 || rows[2].key.pid == 3);
    }

    #[test]
    fn toggling_sort_flips_direction() {
        let mut state = TableState::default();
        let initial = state.descending;
        state.toggle_sort(Column::Cpu);
        assert_ne!(state.descending, initial);
        state.toggle_sort(Column::Cpu);
        assert_eq!(state.descending, initial);
    }

    #[test]
    fn unknown_values_stay_last_when_ascending() {
        let state = TableState {
            sort: Column::Cpu,
            descending: false,
            ..TableState::default()
        };
        let processes = [
            process(1, "a", None),
            process(2, "b", Some(9.0)),
            process(3, "c", None),
        ];
        let mut rows: Vec<&ProcessInfo> = processes.iter().collect();
        sort_rows(&mut rows, &state);
        assert_eq!(rows[0].key.pid, 2);
        assert!(rows[1].key.pid == 1 || rows[1].key.pid == 3);
        assert!(rows[2].key.pid == 1 || rows[2].key.pid == 3);
    }

    #[test]
    fn new_column_starts_descending_for_numbers_only() {
        let mut state = TableState::default();
        state.toggle_sort(Column::Name);
        assert!(!state.descending);
        state.toggle_sort(Column::Memory);
        assert!(state.descending);
    }

    #[test]
    fn column_visibility_toggles() {
        let mut state = TableState::default();
        assert!(state.shows(Column::Cpu));
        state.set_visible(Column::Cpu, false);
        assert!(!state.shows(Column::Cpu));
        state.set_visible(Column::Cpu, true);
        assert!(state.shows(Column::Cpu));
    }

    #[test]
    fn visible_columns_keep_declared_order() {
        let mut state = TableState::default();
        state.set_visible(Column::Cpu, false);
        state.set_visible(Column::IoRead, true);
        let positions: Vec<usize> = state
            .visible
            .iter()
            .map(|column| Column::all().iter().position(|c| c == column).unwrap())
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted);
    }

    #[test]
    fn layout_width_matches_columns() {
        let columns = Column::default_visible().to_vec();
        let layout = Layout::new(&columns);
        let expected: f32 = columns.iter().map(|column| column.width()).sum::<f32>()
            + COLUMN_GAP * (columns.len() as f32 - 1.0);
        assert!((layout.total_width - expected).abs() < 0.01);
        assert_eq!(layout.widths.len(), columns.len());
    }
}
