//! `mcpad-theme` — темы оформления.
//!
//! Тема — один JSON, из которого собираются все три представления: окно на
//! egui, бесконечный canvas и терминал. Применяется в рантайме: тема
//! подставляется прямо в `egui::Visuals` и в палитру canvas, без WebView и
//! без перезапуска приложения.
//!
//! Цвета задаются строкой `#rrggbb` и разбираются в `Rgb`, который умеет
//! отдавать и каналы, и готовую строку.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Ошибка разбора темы.
///
/// `Eq` здесь невозможен: значение метрики — `f32`, а у него нет такого
/// трейта. Сравнение остаётся через `PartialEq`.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeError {
    /// Некорректный JSON.
    Json(String),
    /// Некорректный цвет.
    Color { field: String, value: String },
    /// Значение вне допустимого диапазона.
    Range { field: String, value: f32 },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Json(message) => write!(f, "некорректный JSON темы: {message}"),
            ThemeError::Color { field, value } => {
                write!(f, "некорректный цвет {field}: {value}")
            }
            ThemeError::Range { field, value } => {
                write!(f, "значение {field} вне диапазона: {value}")
            }
        }
    }
}

impl std::error::Error for ThemeError {}

/// Цвет RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Разбирает `#rrggbb` или `rrggbb`.
    pub fn from_hex(value: &str) -> Result<Self, ThemeError> {
        let hex = value.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ThemeError::Color {
                field: "цвет".to_string(),
                value: value.to_string(),
            });
        }
        let red = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ThemeError::Color {
            field: "цвет".to_string(),
            value: value.to_string(),
        })?;
        let green = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ThemeError::Color {
            field: "цвет".to_string(),
            value: value.to_string(),
        })?;
        let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ThemeError::Color {
            field: "цвет".to_string(),
            value: value.to_string(),
        })?;
        Ok(Rgb {
            r: red,
            g: green,
            b: blue,
        })
    }

    /// Цвет как `#rrggbb`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Rgb::from_hex(&raw).map_err(serde::de::Error::custom)
    }
}

/// Палитра темы.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    /// Фон окна.
    #[serde(default = "default_background")]
    pub background: Rgb,
    /// Фон панелей.
    #[serde(default = "default_surface")]
    pub surface: Rgb,
    /// Основной текст.
    #[serde(default = "default_text")]
    pub text: Rgb,
    /// Приглушённый текст.
    #[serde(default = "default_muted")]
    pub muted: Rgb,
    /// Акцент.
    #[serde(default = "default_accent")]
    pub accent: Rgb,
    /// Успех / норма.
    #[serde(default = "default_ok")]
    pub ok: Rgb,
    /// Предупреждение.
    #[serde(default = "default_warning")]
    pub warning: Rgb,
    /// Критичное состояние.
    #[serde(default = "default_critical")]
    pub critical: Rgb,
    /// Сетка canvas.
    #[serde(default = "default_grid")]
    pub grid: Rgb,
    /// Рёбра графа.
    #[serde(default = "default_edge")]
    pub edge: Rgb,
}

fn default_background() -> Rgb {
    Rgb::from_hex("#14161a").expect("константа корректна")
}
fn default_surface() -> Rgb {
    Rgb::from_hex("#1b1f25").expect("константа корректна")
}
fn default_text() -> Rgb {
    Rgb::from_hex("#e6eaf0").expect("константа корректна")
}
fn default_muted() -> Rgb {
    Rgb::from_hex("#9aa6b4").expect("константа корректна")
}
fn default_accent() -> Rgb {
    Rgb::from_hex("#ff8a6b").expect("константа корректна")
}
fn default_ok() -> Rgb {
    Rgb::from_hex("#62d394").expect("константа корректна")
}
fn default_warning() -> Rgb {
    Rgb::from_hex("#e3b341").expect("константа корректна")
}
fn default_critical() -> Rgb {
    Rgb::from_hex("#ff5c5c").expect("константа корректна")
}
fn default_grid() -> Rgb {
    Rgb::from_hex("#232830").expect("константа корректна")
}
fn default_edge() -> Rgb {
    Rgb::from_hex("#4a5563").expect("константа корректна")
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: default_background(),
            surface: default_surface(),
            text: default_text(),
            muted: default_muted(),
            accent: default_accent(),
            ok: default_ok(),
            warning: default_warning(),
            critical: default_critical(),
            grid: default_grid(),
            edge: default_edge(),
        }
    }
}

/// Метрики оформления.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Высота строки таблицы.
    #[serde(default = "default_row_height")]
    pub row_height: f32,
    /// Радиус скругления.
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f32,
    /// Масштаб шрифта.
    #[serde(default = "default_font_scale")]
    pub font_scale: f32,
    /// Радиус узла графа.
    #[serde(default = "default_node_radius")]
    pub node_radius: f32,
    /// Шаг сетки canvas.
    #[serde(default = "default_grid_step")]
    pub grid_step: f32,
}

fn default_row_height() -> f32 {
    22.0
}
fn default_corner_radius() -> f32 {
    8.0
}
fn default_font_scale() -> f32 {
    1.0
}
fn default_node_radius() -> f32 {
    6.0
}
fn default_grid_step() -> f32 {
    32.0
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            row_height: default_row_height(),
            corner_radius: default_corner_radius(),
            font_scale: default_font_scale(),
            node_radius: default_node_radius(),
            grid_step: default_grid_step(),
        }
    }
}

/// Тема оформления.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Имя темы.
    #[serde(default = "default_name")]
    pub name: String,
    /// Тёмная ли тема.
    #[serde(default = "default_true")]
    pub dark: bool,
    /// Палитра.
    #[serde(default)]
    pub palette: Palette,
    /// Метрики.
    #[serde(default)]
    pub metrics: Metrics,
}

fn default_name() -> String {
    "mcpad".to_string()
}
fn default_true() -> bool {
    true
}

impl Theme {
    /// Разбирает тему из JSON-строки.
    pub fn from_json(text: &str) -> Result<Self, ThemeError> {
        serde_json::from_str(text).map_err(|error| ThemeError::Json(error.to_string()))
    }

    /// Разбирает тему из готового значения.
    pub fn from_value(value: serde_json::Value) -> Result<Self, ThemeError> {
        serde_json::from_value(value).map_err(|error| ThemeError::Json(error.to_string()))
    }

    /// Тема по умолчанию — тёмная.
    pub fn default_dark() -> Self {
        Theme {
            name: "mcpad-dark".to_string(),
            dark: true,
            palette: Palette {
                background: default_background(),
                surface: default_surface(),
                text: default_text(),
                muted: default_muted(),
                accent: default_accent(),
                ok: default_ok(),
                warning: default_warning(),
                critical: default_critical(),
                grid: default_grid(),
                edge: default_edge(),
            },
            metrics: Metrics {
                row_height: default_row_height(),
                corner_radius: default_corner_radius(),
                font_scale: default_font_scale(),
                node_radius: default_node_radius(),
                grid_step: default_grid_step(),
            },
        }
    }

    /// Тема по умолчанию — светлая.
    pub fn default_light() -> Self {
        let mut theme = Theme::default_dark();
        theme.name = "mcpad-light".to_string();
        theme.dark = false;
        theme.palette.background = Rgb::from_hex("#f6f7f9").expect("константа корректна");
        theme.palette.surface = Rgb::from_hex("#ffffff").expect("константа корректна");
        theme.palette.text = Rgb::from_hex("#16191d").expect("константа корректна");
        theme.palette.muted = Rgb::from_hex("#5b6472").expect("константа корректна");
        theme.palette.grid = Rgb::from_hex("#d9dee5").expect("константа корректна");
        theme.palette.edge = Rgb::from_hex("#b9c2cd").expect("константа корректна");
        theme
    }

    /// Проверяет метрики на допустимость.
    pub fn validate(&self) -> Result<(), ThemeError> {
        let checks = [
            ("row_height", self.metrics.row_height),
            ("corner_radius", self.metrics.corner_radius),
            ("font_scale", self.metrics.font_scale),
            ("node_radius", self.metrics.node_radius),
            ("grid_step", self.metrics.grid_step),
        ];
        for (field, value) in checks {
            if !(value.is_finite() && value > 0.0 && value <= 128.0) {
                return Err(ThemeError::Range {
                    field: field.to_string(),
                    value,
                });
            }
        }
        Ok(())
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::default_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Два решётки: внутри темы есть цвета вида "#rrggbb", а одиночная
    // решётка завершала бы raw-строку раньше времени.
    const DARK: &str = r##"{
      "name": "mcpad-dark",
      "dark": true,
      "palette": {
        "background": "#14161a",
        "surface": "#1b1f25",
        "text": "#e6eaf0",
        "accent": "#ff8a6b"
      },
      "metrics": { "row_height": 24.0, "grid_step": 40.0 }
    }"##;

    #[test]
    fn parses_theme() {
        let theme = Theme::from_json(DARK).expect("тема разобрана");
        assert_eq!(theme.name, "mcpad-dark");
        assert!(theme.dark);
        assert_eq!(theme.palette.background, Rgb::from_hex("#14161a").unwrap());
        assert_eq!(theme.palette.accent.to_hex(), "#ff8a6b");
        assert_eq!(theme.metrics.row_height, 24.0);
        assert_eq!(theme.metrics.grid_step, 40.0);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let theme = Theme::from_json(r#"{"name": "minimal"}"#).expect("тема разобрана");
        assert_eq!(theme.metrics.row_height, 22.0);
        assert_eq!(theme.palette.background, default_background());
        assert!(theme.dark);
    }

    #[test]
    fn broken_color_is_reported() {
        let error = Theme::from_json(r##"{"palette": {"accent": "#zzz"}}"##).unwrap_err();
        assert!(matches!(error, ThemeError::Json(_)));
    }

    #[test]
    fn invalid_metrics_are_rejected() {
        let mut theme = Theme::default_dark();
        theme.metrics.row_height = 0.0;
        assert!(matches!(theme.validate(), Err(ThemeError::Range { .. })));
    }

    #[test]
    fn light_theme_differs_from_dark() {
        let dark = Theme::default_dark();
        let light = Theme::default_light();
        assert_ne!(dark.palette.background, light.palette.background);
        assert!(!light.dark);
        assert!(light.validate().is_ok());
    }

    #[test]
    fn theme_round_trips_through_json() {
        let theme = Theme::default_dark();
        let json = serde_json::to_string(&theme).expect("тема сериализуется");
        let restored = Theme::from_json(&json).expect("тема разбирается обратно");
        assert_eq!(theme, restored);
    }
}
