//! Тепловая карта: чем нагружен процесс, тем «горячее» его ячейка.
//!
//! Идея взята из Диспетчера задач Windows 11: цветом в колонке показывают
//! нагрузку, чтобы тяжёлый процесс находился глазом, а не чтением чисел.
//!
//! Важное правило: **неизвестное значение не красится**. Если доли нет —
//! ячейка остаётся нейтральной, а не «холодной»: иначе «нет данных» и
//! «нулевая нагрузка» выглядели бы одинаково.

use egui::Color32;
use mcpad_theme::{Rgb, Theme};

/// Уровень нагрузки для окраски ячейки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heat {
    /// Нагрузки нет или она неизвестна.
    None,
    /// Малая.
    Low,
    /// Заметная.
    Mid,
    /// Высокая.
    High,
    /// Критическая.
    Critical,
}

/// Ниже этой доли ячейка не окрашивается вовсе.
const SILENT: f64 = 0.02;
/// Граница «малой» нагрузки.
const LOW: f64 = 0.15;
/// Граница «заметной» нагрузки.
const MID: f64 = 0.40;
/// Граница «высокой» нагрузки.
const HIGH: f64 = 0.70;

impl Heat {
    /// Классифицирует долю нагрузки.
    ///
    /// `None` — доля неизвестна: красить нечего.
    pub fn of(ratio: Option<f64>) -> Heat {
        let Some(ratio) = ratio else {
            return Heat::None;
        };
        if !ratio.is_finite() || ratio <= SILENT {
            Heat::None
        } else if ratio < LOW {
            Heat::Low
        } else if ratio < MID {
            Heat::Mid
        } else if ratio < HIGH {
            Heat::High
        } else {
            Heat::Critical
        }
    }

    /// Прозрачность подложки ячейки.
    fn alpha(self) -> u8 {
        match self {
            Heat::None => 0,
            Heat::Low => 34,
            Heat::Mid => 56,
            Heat::High => 78,
            Heat::Critical => 100,
        }
    }

    /// Цвет подложки ячейки.
    pub fn fill(self, theme: &Theme) -> Color32 {
        let alpha = self.alpha();
        if alpha == 0 {
            return Color32::TRANSPARENT;
        }
        let rgb = self.tint(theme);
        Color32::from_rgba_unmultiplied(rgb.r, rgb.g, rgb.b, alpha)
    }

    /// Цвет текста поверх такой ячейки.
    pub fn text(self, theme: &Theme) -> Color32 {
        match self {
            Heat::None => rgb_to_color(theme.palette.text),
            Heat::Low => rgb_to_color(theme.palette.ok),
            Heat::Mid => rgb_to_color(theme.palette.warning),
            Heat::High => rgb_to_color(mix(theme.palette.warning, theme.palette.critical, 0.5)),
            Heat::Critical => rgb_to_color(theme.palette.critical),
        }
    }

    /// Оттенок заливки.
    fn tint(self, theme: &Theme) -> Rgb {
        match self {
            Heat::None => theme.palette.surface,
            Heat::Low => theme.palette.ok,
            Heat::Mid => theme.palette.warning,
            Heat::High => mix(theme.palette.warning, theme.palette.critical, 0.5),
            Heat::Critical => theme.palette.critical,
        }
    }
}

/// Цвет темы в цвет egui.
pub fn rgb_to_color(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Смешивает два цвета: `t` — доля второго.
pub fn mix(first: Rgb, second: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    Rgb {
        r: (first.r as f64 + (second.r as f64 - first.r as f64) * t).round() as u8,
        g: (first.g as f64 + (second.g as f64 - first.g as f64) * t).round() as u8,
        b: (first.b as f64 + (second.b as f64 - first.b as f64) * t).round() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_ratio_is_not_painted() {
        assert_eq!(Heat::of(None), Heat::None);
    }

    #[test]
    fn zero_load_is_not_painted() {
        assert_eq!(Heat::of(Some(0.0)), Heat::None);
    }

    #[test]
    fn levels_grow_with_load() {
        assert_eq!(Heat::of(Some(0.05)), Heat::Low);
        assert_eq!(Heat::of(Some(0.30)), Heat::Mid);
        assert_eq!(Heat::of(Some(0.60)), Heat::High);
        assert_eq!(Heat::of(Some(0.95)), Heat::Critical);
    }

    #[test]
    fn out_of_range_ratio_is_critical() {
        assert_eq!(Heat::of(Some(4.0)), Heat::Critical);
    }

    #[test]
    fn nan_ratio_is_not_painted() {
        assert_eq!(Heat::of(Some(f64::NAN)), Heat::None);
    }

    #[test]
    fn unknown_has_transparent_fill() {
        let theme = Theme::default_dark();
        assert_eq!(Heat::None.fill(&theme), Color32::TRANSPARENT);
    }

    #[test]
    fn critical_fill_is_more_opaque_than_low() {
        let theme = Theme::default_dark();
        let low = Heat::Low.fill(&theme).a();
        let critical = Heat::Critical.fill(&theme).a();
        assert!(critical > low);
        assert!(low > 0);
    }

    #[test]
    fn mixing_interpolates_channels() {
        let black = Rgb { r: 0, g: 0, b: 0 };
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let middle = mix(black, white, 0.5);
        assert_eq!(middle.r, 128);
        assert_eq!(middle.g, 128);
        assert_eq!(middle.b, 128);
    }

    #[test]
    fn mixing_clamps_out_of_range() {
        let black = Rgb { r: 0, g: 0, b: 0 };
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        assert_eq!(mix(black, white, 2.0).r, 255);
        assert_eq!(mix(black, white, -1.0).r, 0);
    }
}
