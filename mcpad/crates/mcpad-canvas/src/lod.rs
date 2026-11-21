//! Уровни детализации canvas.
//!
//! На графе в десять тысяч узлов подписи не читаются: текст в мелком масштабе
//! превращается в кашу, а отрисовка тысяч строк стоит дорого. Поэтому рисунок
//! упрощается по мере отдаления:
//!
//! * **Full** — рамка, подпись и вторая строка;
//! * **Boxes** — рамка без подписей;
//! * **Points** — только точка в центре.
//!
//! Правила вынесены отдельно от egui, чтобы их можно было проверить тестами.

/// Насколько подробно рисовать граф.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// Только точки: рамки слились бы в кашу.
    Points,
    /// Рамки без подписей.
    Boxes,
    /// Рамки с подписями.
    Full,
}

impl Detail {
    /// Рисовать ли подписи узлов.
    pub fn has_labels(self) -> bool {
        matches!(self, Detail::Full)
    }

    /// Рисовать ли рамки узлов.
    pub fn has_boxes(self) -> bool {
        matches!(self, Detail::Boxes | Detail::Full)
    }
}

/// Ниже этого масштаба подписи нечитаемы.
pub const LABEL_MIN_SCALE: f32 = 0.55;

/// Ниже этого масштаба рамки сливаются в сплошную заливку.
pub const BOX_MIN_SCALE: f32 = 0.22;

/// Уровень детализации по масштабу.
pub fn detail_for_scale(scale: f32) -> Detail {
    if !scale.is_finite() {
        return Detail::Points;
    }
    if scale >= LABEL_MIN_SCALE {
        Detail::Full
    } else if scale >= BOX_MIN_SCALE {
        Detail::Boxes
    } else {
        Detail::Points
    }
}

/// Размер шрифта подписи при заданном масштабе.
///
/// Шрифт не уменьшается бесконечно: ниже определённого размера он всё равно
/// нечитаем, поэтому его лучше не рисовать вовсе.
pub fn label_size(scale: f32) -> f32 {
    if !scale.is_finite() {
        return 11.0;
    }
    (11.0 * scale).clamp(9.0, 16.0)
}

/// Размер точки в режиме `Points`.
pub fn point_size(scale: f32) -> f32 {
    if !scale.is_finite() {
        return 2.0;
    }
    (3.0 * scale).clamp(1.5, 6.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_scale_keeps_labels() {
        assert_eq!(detail_for_scale(1.0), Detail::Full);
        assert_eq!(detail_for_scale(0.55), Detail::Full);
        assert!(detail_for_scale(1.0).has_labels());
    }

    #[test]
    fn middle_scale_drops_labels() {
        assert_eq!(detail_for_scale(0.4), Detail::Boxes);
        assert!(!detail_for_scale(0.4).has_labels());
        assert!(detail_for_scale(0.4).has_boxes());
    }

    #[test]
    fn far_scale_drops_boxes() {
        assert_eq!(detail_for_scale(0.1), Detail::Points);
        assert!(!detail_for_scale(0.1).has_boxes());
    }

    #[test]
    fn broken_scale_is_treated_as_far() {
        assert_eq!(detail_for_scale(f32::NAN), Detail::Points);
        assert_eq!(detail_for_scale(f32::INFINITY), Detail::Points);
    }

    #[test]
    fn label_size_is_clamped() {
        assert!(label_size(0.1) >= 9.0);
        assert!(label_size(8.0) <= 16.0);
        assert!(label_size(1.0) > label_size(0.5));
    }
}
