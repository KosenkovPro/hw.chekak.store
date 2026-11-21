//! Применение JSON-темы к egui.
//!
//! Тема приходит одним файлом и раздаётся трём представлениям: окну egui,
//! canvas и терминалу. Здесь она переводится в [`egui::Visuals`] и применяется
//! прямо во время работы — без перезапуска приложения.
//!
//! Размеры шрифтов задаются абсолютно, а не умножением на текущие: иначе
//! повторная загрузка темы с масштабом 1.2 накапливала бы множитель.

use egui::{Color32, Context, FontId, Stroke, TextStyle, Visuals};
use mcpad_theme::{Rgb, Theme};

/// Цвет темы в цвет egui.
fn color(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Базовые шрифты egui, к которым применяется `font_scale` темы.
fn base_fonts() -> Vec<(TextStyle, FontId)> {
    let style = egui::Style::default();
    style
        .text_styles
        .iter()
        .map(|(text_style, font)| (text_style.clone(), font.clone()))
        .collect()
}

/// Собирает `egui::Visuals` по теме.
pub fn visuals_for(theme: &Theme) -> Visuals {
    let palette = &theme.palette;
    let mut visuals = if theme.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let background = color(palette.background);
    let surface = color(palette.surface);
    let text = color(palette.text);
    let muted = color(palette.muted);
    let accent = color(palette.accent);

    visuals.override_text_color = Some(text);
    visuals.panel_fill = background;
    visuals.window_fill = surface;
    visuals.extreme_bg_color = background;
    visuals.faint_bg_color = surface;
    visuals.hyperlink_color = accent;
    visuals.warn_fg_color = color(palette.warning);
    visuals.error_fg_color = color(palette.critical);
    visuals.striped = false;

    visuals.selection.bg_fill = accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, accent);

    visuals.widgets.noninteractive.bg_fill = surface;
    visuals.widgets.noninteractive.weak_bg_fill = background;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, muted);

    visuals.widgets.inactive.bg_fill = surface;
    visuals.widgets.inactive.weak_bg_fill = background;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, muted);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, muted);

    visuals.widgets.hovered.bg_fill = surface;
    visuals.widgets.hovered.weak_bg_fill = background;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, accent);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent);

    visuals.widgets.active.bg_fill = accent;
    visuals.widgets.active.weak_bg_fill = accent;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, background);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent);

    visuals.widgets.open.bg_fill = surface;
    visuals.widgets.open.weak_bg_fill = background;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, text);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, text);

    visuals
}

/// Применяет тему к контексту: цвета и размеры шрифтов.
pub fn apply(ctx: &Context, theme: &Theme) {
    let visuals = visuals_for(theme);
    let scale = theme.metrics.font_scale;

    ctx.set_visuals(visuals);
    // Тема egui следует за тем, тёмная или светлая тема MCPad.
    ctx.set_theme(if theme.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|style| {
        for (text_style, font) in base_fonts() {
            let size = (font.size * scale).round().max(8.0);
            style.text_styles.insert(
                text_style,
                FontId {
                    family: font.family,
                    size,
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_theme::Rgb;

    #[test]
    fn palette_is_transferred_to_visuals() {
        let mut theme = Theme::default_dark();
        theme.palette.background = Rgb::from_hex("#101010").expect("цвет корректен");
        theme.palette.accent = Rgb::from_hex("#ff0000").expect("цвет корректен");

        let visuals = visuals_for(&theme);
        assert_eq!(visuals.panel_fill, Color32::from_rgb(16, 16, 16));
        assert_eq!(visuals.hyperlink_color, Color32::from_rgb(255, 0, 0));
        assert_eq!(visuals.override_text_color, Some(color(theme.palette.text)));
    }

    #[test]
    fn light_theme_gives_light_visuals() {
        let theme = Theme::default_light();
        let visuals = visuals_for(&theme);
        assert!(!visuals.dark_mode);
        assert_eq!(visuals.panel_fill, color(theme.palette.background));
    }

    #[test]
    fn dark_theme_gives_dark_visuals() {
        let theme = Theme::default_dark();
        let visuals = visuals_for(&theme);
        assert!(visuals.dark_mode);
    }

    #[test]
    fn font_scale_changes_text_size() {
        let context = Context::default();
        let mut theme = Theme::default_dark();
        theme.metrics.font_scale = 2.0;
        apply(&context, &theme);

        let size = context.style_of(egui::Theme::Dark).text_styles[&TextStyle::Body].size;
        let base = egui::Style::default().text_styles[&TextStyle::Body].size;
        assert_eq!(size, (base * 2.0).round());
    }

    #[test]
    fn repeated_apply_does_not_compound_scale() {
        let context = Context::default();
        let mut theme = Theme::default_dark();
        theme.metrics.font_scale = 1.5;
        apply(&context, &theme);
        let first = context.style_of(egui::Theme::Dark).text_styles[&TextStyle::Body].size;
        apply(&context, &theme);
        let second = context.style_of(egui::Theme::Dark).text_styles[&TextStyle::Body].size;
        assert_eq!(first, second);
    }
}
