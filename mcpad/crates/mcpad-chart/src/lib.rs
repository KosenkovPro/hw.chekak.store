//! `mcpad-chart` — графики измерений.
//!
//! Оформление сознательно спокойное: без осей и подписей, с тонированной
//! сеткой, шкалой над графиком и временем под ним. Ряды с пропусками
//! разрываются: провал в данных не должен выглядеть как провал до нуля.

use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use mcpad_series::Sample;
use mcpad_theme::Theme;

/// Описание графика.
#[derive(Debug, Clone)]
pub struct ChartSpec {
    /// Заголовок.
    pub title: String,
    /// Единица измерения.
    pub unit: String,
    /// Цвет линии.
    pub color: Color32,
    /// Заливать ли area под линией.
    pub filled: bool,
}

impl ChartSpec {
    pub fn new(title: impl Into<String>, unit: impl Into<String>, color: Color32) -> Self {
        Self {
            title: title.into(),
            unit: unit.into(),
            color,
            filled: true,
        }
    }
}

/// Границы шкалы значений.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    pub min: f64,
    pub max: f64,
}

impl Scale {
    /// Шкала по ряду. Пустой ряд показывается как 0..100.
    pub fn from_samples(samples: &[&Sample]) -> Scale {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for sample in samples {
            if let Some(value) = sample.value {
                min = min.min(value);
                max = max.max(value);
            }
        }

        if !min.is_finite() || !max.is_finite() {
            return Scale {
                min: 0.0,
                max: 100.0,
            };
        }
        if (max - min).abs() < f64::EPSILON {
            // Ровная линия: показываем окно вокруг значения, а не ноль-в-ноль.
            let pad = if max.abs() < 1.0 {
                1.0
            } else {
                max.abs() * 0.1
            };
            return Scale {
                min: (min - pad).max(0.0),
                max: max + pad,
            };
        }

        Scale { min, max }
    }
}

/// Точка в координатах графика.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotPoint {
    pub x: f32,
    pub y: f32,
}

/// Переводит отсчёты в точки внутри прямоугольника.
///
/// Пропуски (`value == None`) в точки не превращаются: они разрывают линию.
pub fn plot_points(
    samples: &[&Sample],
    rect: Rect,
    scale: Scale,
    from: u64,
    to: u64,
) -> Vec<PlotPoint> {
    let span = to.saturating_sub(from).max(1) as f64;
    let height = (scale.max - scale.min).max(f64::EPSILON);

    samples
        .iter()
        .filter_map(|sample| {
            let value = sample.value?;
            let x = rect.min.x
                + (sample.at_unix_ms.saturating_sub(from) as f64 / span) as f32 * rect.width();
            let y = rect.max.y - ((value - scale.min) / height) as f32 * rect.height();
            Some(PlotPoint { x, y })
        })
        .collect()
}

/// Рисует график.
pub fn plot_series(
    ui: &mut Ui,
    rect: Rect,
    samples: &[&Sample],
    spec: &ChartSpec,
    theme: &Theme,
    from: u64,
    to: u64,
) {
    let painter = ui.painter().with_clip_rect(rect);
    let grid_color = Color32::from_rgb(
        theme.palette.grid.r,
        theme.palette.grid.g,
        theme.palette.grid.b,
    );
    let text_color = Color32::from_rgb(
        theme.palette.text.r,
        theme.palette.text.g,
        theme.palette.text.b,
    );
    let muted_color = Color32::from_rgb(
        theme.palette.muted.r,
        theme.palette.muted.g,
        theme.palette.muted.b,
    );

    // Тонированная сетка: четыре горизонтальные линии.
    for step in 0..4 {
        let y = rect.min.y + rect.height() * step as f32 / 3.0;
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(1.0, grid_color),
        );
    }

    let scale = Scale::from_samples(samples);
    let points = plot_points(samples, rect, scale, from, to);

    if points.len() >= 2 {
        if spec.filled {
            let mut fill = Vec::with_capacity(points.len() + 2);
            fill.push(Pos2::new(points[0].x, rect.max.y));
            for point in &points {
                fill.push(Pos2::new(point.x, point.y));
            }
            fill.push(Pos2::new(points[points.len() - 1].x, rect.max.y));

            // Полупрозрачная заливка под линией.
            let background =
                Color32::from_rgba_unmultiplied(spec.color.r(), spec.color.g(), spec.color.b(), 48);
            painter.add(egui::Shape::convex_polygon(fill, background, Stroke::NONE));
        }

        let path: Vec<Pos2> = points
            .iter()
            .map(|point| Pos2::new(point.x, point.y))
            .collect();
        painter.add(egui::Shape::line(path, Stroke::new(1.5, spec.color)));
    }

    // Шкала над графиком и время под ним.
    let span_text = format!("{:.1}–{:.1} {}", scale.min, scale.max, spec.unit);
    painter.text(
        rect.left_top() + Vec2::new(0.0, -4.0),
        egui::Align2::LEFT_TOP,
        &span_text,
        egui::FontId::monospace(11.0),
        text_color,
    );

    let time_text = format!("{} с", (to.saturating_sub(from)) / 1000);
    painter.text(
        rect.left_bottom() + Vec2::new(0.0, 4.0),
        egui::Align2::LEFT_BOTTOM,
        time_text,
        egui::FontId::monospace(11.0),
        muted_color,
    );
}

/// Рисует курсор по оси времени.
pub fn plot_cursor(
    ui: &mut Ui,
    rect: Rect,
    cursor_unix_ms: u64,
    from: u64,
    to: u64,
    theme: &Theme,
) {
    let span = to.saturating_sub(from).max(1) as f64;
    let x = rect.min.x + (cursor_unix_ms.saturating_sub(from) as f64 / span) as f32 * rect.width();
    if x < rect.min.x || x > rect.max.x {
        return;
    }

    let accent = Color32::from_rgb(
        theme.palette.accent.r,
        theme.palette.accent.g,
        theme.palette.accent.b,
    );
    let painter = ui.painter().with_clip_rect(rect);
    painter.line_segment(
        [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
        Stroke::new(1.0, accent),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<Sample> {
        vec![
            Sample::measured(1_000, 10.0),
            Sample::measured(2_000, 30.0),
            Sample::gap(3_000),
            Sample::measured(4_000, 20.0),
        ]
    }

    #[test]
    fn scale_uses_measured_values_only() {
        let all = samples();
        let refs: Vec<&Sample> = all.iter().collect();
        let scale = Scale::from_samples(&refs);
        assert_eq!(scale.min, 10.0);
        assert_eq!(scale.max, 30.0);
    }

    #[test]
    fn empty_series_falls_back_to_full_scale() {
        let scale = Scale::from_samples(&[]);
        assert_eq!(scale.min, 0.0);
        assert_eq!(scale.max, 100.0);
    }

    #[test]
    fn flat_line_gets_padding() {
        let flat = [Sample::measured(1, 50.0), Sample::measured(2, 50.0)];
        let refs: Vec<&Sample> = flat.iter().collect();
        let scale = Scale::from_samples(&refs);
        assert!(scale.max > 50.0);
        assert!(scale.min < 50.0);
    }

    #[test]
    fn gaps_break_the_line() {
        let all = samples();
        let refs: Vec<&Sample> = all.iter().collect();
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 50.0));
        let points = plot_points(&refs, rect, Scale::from_samples(&refs), 1_000, 4_000);
        // Четыре отсчёта, один из них — пропуск: в линию попадают три точки.
        assert_eq!(points.len(), 3);
    }

    #[test]
    fn points_stay_inside_rect() {
        let all = samples();
        let refs: Vec<&Sample> = all.iter().collect();
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        let points = plot_points(&refs, rect, Scale::from_samples(&refs), 1_000, 4_000);
        for point in points {
            assert!(point.x >= rect.min.x - 0.001 && point.x <= rect.max.x + 0.001);
            assert!(point.y >= rect.min.y - 0.001 && point.y <= rect.max.y + 0.001);
        }
    }
}
