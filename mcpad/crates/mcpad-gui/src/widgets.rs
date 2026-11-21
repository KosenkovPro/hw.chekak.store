//! Мелкие элементы интерфейса: карточки метрик, шкалы, мини-графики, значки.
//!
//! Всё это — тонкие обёртки над egui, собранные в одном месте, чтобы
//! представления выглядели одинаково: одинаковые скругления, отступы, размеры
//! шрифта и толщина линий.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use mcpad_theme::Theme;

use crate::heat::rgb_to_color;

/// Скругление углов карточек и панелей.
pub const RADIUS: f32 = 10.0;

/// Карточка метрики: подпись, крупное значение и шкала загрузки.
pub struct StatCard<'a> {
    /// Подпись над значением.
    pub label: &'a str,
    /// Крупное значение.
    pub value: &'a str,
    /// Доля загрузки для шкалы; `None` — шкалы нет.
    pub ratio: Option<f64>,
    /// Цвет шкалы и значения.
    pub color: Color32,
}

/// Рисует карточку метрики.
pub fn stat_card(ui: &mut egui::Ui, theme: &Theme, card: StatCard<'_>) {
    let surface = rgb_to_color(theme.palette.surface);
    let edge = rgb_to_color(theme.palette.edge);
    let muted = rgb_to_color(theme.palette.muted);
    let text = rgb_to_color(theme.palette.text);

    egui::Frame::new()
        .fill(surface)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(RADIUS)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(card.label).size(11.0).color(muted));
            ui.label(RichText::new(card.value).size(22.0).strong().color(text));
            if let Some(ratio) = card.ratio {
                let ratio = if ratio.is_finite() {
                    ratio.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(ratio as f32)
                        .fill(card.color)
                        .desired_height(5.0),
                );
            }
        });
}

/// Значок-«таблетка» с короткой подписью: CPU, память, число процессов.
pub fn chip(ui: &mut egui::Ui, theme: &Theme, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(rgb_to_color(theme.palette.surface))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}

/// Заголовок раздела с тонкой линией под ним.
pub fn section_title(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    let muted = rgb_to_color(theme.palette.muted);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(muted),
    );
    ui.add_space(2.0);
}

/// Стрелка «свёрнуто/развёрнуто» для дерева процессов.
///
/// Рисуется линиями, а не символом шрифта: одинаково выглядит на любой
/// системе и не зависит от наличия символов в шрифте.
pub fn chevron(painter: &egui::Painter, rect: Rect, expanded: bool, color: Color32) {
    let stroke = Stroke::new(1.4, color);
    let center = rect.center();

    if expanded {
        // Стрелка вниз.
        painter.line_segment(
            [
                Pos2::new(center.x - 4.0, center.y - 2.0),
                Pos2::new(center.x, center.y + 2.0),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(center.x, center.y + 2.0),
                Pos2::new(center.x + 4.0, center.y - 2.0),
            ],
            stroke,
        );
    } else {
        // Стрелка вправо.
        painter.line_segment(
            [
                Pos2::new(center.x - 2.0, center.y - 4.0),
                Pos2::new(center.x + 2.0, center.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(center.x + 2.0, center.y),
                Pos2::new(center.x - 2.0, center.y + 4.0),
            ],
            stroke,
        );
    }
}

/// Мини-график ряда значений.
///
/// Пропуски разрывают линию: если измерения не было, на графике это разрыв,
/// а не ноль.
pub fn sparkline(
    ui: &mut egui::Ui,
    samples: &[Option<f64>],
    color: Color32,
    size: Vec2,
    grid: Color32,
) {
    let (rect, _response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    sparkline_in_rect(&painter, rect, samples, color, grid);
}

/// Мини-график внутри готового прямоугольника.
///
/// Отдельная функция нужна таблице: там ячейка уже размечена, а график должен
/// попасть внутрь неё, а не занять новую строку.
pub fn sparkline_in_rect(
    painter: &egui::Painter,
    rect: Rect,
    samples: &[Option<f64>],
    color: Color32,
    grid: Color32,
) {
    let painter = painter.with_clip_rect(rect);

    let mut max = 0.0f64;
    for value in samples.iter().flatten() {
        max = max.max(*value);
    }
    if max <= 0.0 {
        max = 1.0;
    }

    // Базовая линия: без неё пустой ряд выглядит как «ноль».
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, grid),
    );

    let count = samples.len();
    if count < 2 {
        return;
    }

    let step = rect.width() / (count - 1) as f32;
    let mut previous: Option<Pos2> = None;
    for (index, sample) in samples.iter().enumerate() {
        let x = rect.min.x + step * index as f32;
        let Some(value) = sample else {
            // Измерения не было — линия прерывается.
            previous = None;
            continue;
        };
        let ratio = (value / max).clamp(0.0, 1.0) as f32;
        let point = Pos2::new(x, rect.max.y - ratio * rect.height());
        if let Some(previous) = previous {
            painter.line_segment([previous, point], Stroke::new(1.3, color));
        }
        previous = Some(point);
    }
}

/// Значок представления в боковой навигации.
///
/// Иконки рисуются линиями, а не берутся из шрифта: так они выглядят одинаково
/// на любой системе и не зависят от наличия символов в шрифте.
pub fn nav_icon(painter: &egui::Painter, rect: Rect, kind: NavIcon, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let mid_x = rect.center().x;
    let mid_y = rect.center().y;

    match kind {
        NavIcon::Processes => {
            // Три строки — список процессов.
            for row in 0..3 {
                let y = rect.min.y + rect.height() * (0.22 + 0.28 * row as f32);
                painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], stroke);
            }
        }
        NavIcon::Tree => {
            // Узел с двумя потомками.
            painter.circle_filled(Pos2::new(mid_x, rect.min.y + 3.0), 2.2, color);
            painter.line_segment(
                [Pos2::new(mid_x, rect.min.y + 5.5), Pos2::new(mid_x, mid_y)],
                stroke,
            );
            for offset in [-0.28f32, 0.28f32] {
                let x = mid_x + rect.width() * offset;
                painter.line_segment([Pos2::new(mid_x, mid_y), Pos2::new(x, mid_y)], stroke);
                painter.line_segment(
                    [Pos2::new(x, mid_y), Pos2::new(x, rect.max.y - 2.0)],
                    stroke,
                );
                painter.circle_filled(Pos2::new(x, rect.max.y - 2.0), 2.0, color);
            }
        }
        NavIcon::Performance => {
            // Столбики нарастающей высоты.
            for column in 0..3 {
                let x = rect.min.x + rect.width() * (0.22 + 0.28 * column as f32);
                let height = rect.height() * (0.35 + 0.25 * column as f32);
                painter.line_segment(
                    [Pos2::new(x, rect.max.y), Pos2::new(x, rect.max.y - height)],
                    stroke,
                );
            }
        }
        NavIcon::Canvas => {
            // Узлы графа, соединённые линиями.
            painter.circle_filled(Pos2::new(rect.min.x + 4.0, mid_y), 2.2, color);
            painter.circle_filled(Pos2::new(rect.max.x - 4.0, rect.min.y + 4.0), 2.2, color);
            painter.circle_filled(Pos2::new(rect.max.x - 4.0, rect.max.y - 4.0), 2.2, color);
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 6.0, mid_y),
                    Pos2::new(rect.max.x - 6.0, rect.min.y + 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 6.0, mid_y),
                    Pos2::new(rect.max.x - 6.0, rect.max.y - 4.0),
                ],
                stroke,
            );
        }
        NavIcon::Capabilities => {
            // Щит: у платформы есть набор возможностей.
            painter.line_segment(
                [
                    Pos2::new(mid_x, rect.min.y + 2.0),
                    Pos2::new(rect.max.x - 2.0, rect.min.y + 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.max.x - 2.0, rect.min.y + 5.0),
                    Pos2::new(rect.max.x - 2.0, mid_y + 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.max.x - 2.0, mid_y + 1.0),
                    Pos2::new(mid_x, rect.max.y - 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(mid_x, rect.max.y - 1.0),
                    Pos2::new(rect.min.x + 2.0, mid_y + 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 2.0, mid_y + 1.0),
                    Pos2::new(rect.min.x + 2.0, rect.min.y + 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + 2.0, rect.min.y + 5.0),
                    Pos2::new(mid_x, rect.min.y + 2.0),
                ],
                stroke,
            );
        }
    }
}

/// Значок представления.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavIcon {
    /// Список процессов.
    Processes,
    /// Дерево процессов.
    Tree,
    /// Графики производительности.
    Performance,
    /// Canvas-граф.
    Canvas,
    /// Возможности платформы.
    Capabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_distinct() {
        assert_ne!(NavIcon::Processes, NavIcon::Tree);
        assert_ne!(NavIcon::Performance, NavIcon::Canvas);
        assert_ne!(NavIcon::Canvas, NavIcon::Capabilities);
    }

    #[test]
    fn radius_is_positive() {
        let radius = [RADIUS];
        assert!(radius[0] > 0.0);
    }
}
