//! Отрисовка бесконечного canvas на egui Painter.
//!
//! Canvas показывает граф в двух режимах: дерево процессов и зависимости
//! модулей. Рисование идёт через `Painter`, поэтому упирается в GPU, а не в
//! виджеты.
//!
//! Три вещи удерживают картинку вменяемой на больших графах:
//!
//! * **пространственный индекс** — на кадр рисуются только узлы, попавшие в
//!   видимую область; остальные не перебираются вообще;
//! * **уровни детализации** — на мелком масштабе подписи убираются, на
//!   совсем мелком остаются только точки;
//! * **миникарта** — показывает, где мы находимся в графе целиком.

use std::collections::{HashMap, HashSet};

use egui::{Align2, Color32, FontId, Pos2, Rect as EguiRect, Stroke, Ui, Vec2};
use mcpad_theme::{Rgb, Theme};

use crate::Graph;
use crate::index::SpatialIndex;
use crate::layout::{self, NodeLayout};
use crate::lod::{self, Detail};

/// Ширина узла и зазор между колонками: по ним строится пространственный
/// индекс. Высота и вертикальный зазор заданы в раскладке графа.
const NODE_WIDTH: f32 = 150.0;
const GAP_X: f32 = 70.0;

/// Состояние canvas: смещение, масштаб и выделение.
#[derive(Debug, Clone)]
pub struct CanvasState {
    /// Смещение в экранных координатах.
    pub offset: Pos2,
    /// Масштаб мира.
    pub scale: f32,
    /// Выделенный узел.
    pub selected: Option<u32>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            offset: Pos2::new(40.0, 40.0),
            scale: 1.0,
            selected: None,
        }
    }
}

impl CanvasState {
    /// Мировые координаты → экранные.
    pub fn to_screen(&self, world: Pos2) -> Pos2 {
        Pos2::new(
            world.x * self.scale + self.offset.x,
            world.y * self.scale + self.offset.y,
        )
    }

    /// Экранные координаты → мировые.
    pub fn to_world(&self, screen: Pos2) -> Pos2 {
        Pos2::new(
            (screen.x - self.offset.x) / self.scale,
            (screen.y - self.offset.y) / self.scale,
        )
    }

    /// Прямоугольник видимой области в мировых координатах.
    pub fn viewport(&self, screen: EguiRect) -> layout::Rect {
        let origin = self.to_world(screen.min);
        let size = screen.size() / self.scale;
        layout::Rect {
            x: origin.x,
            y: origin.y,
            width: size.x,
            height: size.y,
        }
    }

    /// Вписывает граф в экран целиком.
    pub fn zoom_to_fit(&mut self, bounds: &layout::Rect, screen: EguiRect) {
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return;
        }
        let scale_x = screen.width() / bounds.width;
        let scale_y = screen.height() / bounds.height;
        // Небольшой запас, чтобы края графа не липли к краю окна.
        self.scale = (scale_x.min(scale_y) * 0.9).clamp(0.1, 8.0);

        let center = Pos2::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        );
        let screen_center = screen.center();
        self.offset = Pos2::new(
            screen_center.x - center.x * self.scale,
            screen_center.y - center.y * self.scale,
        );
    }

    /// Сбрасывает масштаб и смещение.
    pub fn reset(&mut self) {
        self.offset = Pos2::new(40.0, 40.0);
        self.scale = 1.0;
    }
}

/// Рисует canvas.
pub fn show(ui: &mut Ui, state: &mut CanvasState, graph: &Graph, theme: &Theme) {
    let palette = &theme.palette;
    let layout_nodes = graph.layout();

    let (screen_rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

    let background = color(palette.background);
    let painter = ui.painter().with_clip_rect(screen_rect);
    painter.rect_filled(screen_rect, 0.0, background);

    // Индекс строится один раз на кадр, запросы к нему — O(1) на узел.
    let index = build_index(&layout_nodes);
    let viewport = state.viewport(screen_rect);
    let visible: HashSet<u32> = index.query(&viewport).into_iter().collect();

    draw_grid(&painter, state, screen_rect, theme);
    draw_edges(&painter, state, &layout_nodes, graph, &visible, theme);
    draw_nodes(&painter, state, &layout_nodes, graph, &visible, theme);
    draw_minimap(&painter, state, &layout_nodes, screen_rect, theme);

    // Панорамирование: тянем мышью.
    if response.dragged() {
        state.offset += response.drag_delta();
    }

    // Масштаб: колесо мыши, с сохранением точки под курсором.
    let scroll = ui.input(|input| input.smooth_scroll_delta.y);
    if scroll.abs() > 0.01 {
        let before = state.to_world(response.hover_pos().unwrap_or(screen_rect.center()));
        let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
        state.scale = (state.scale * factor).clamp(0.1, 8.0);
        let after = state.to_world(response.hover_pos().unwrap_or(screen_rect.center()));
        state.offset += (before - after) * state.scale;
    }

    // Выделение узла.
    if response.clicked() {
        let world = state.to_world(response.hover_pos().unwrap_or(screen_rect.center()));
        state.selected = layout::node_at(&layout_nodes, (world.x, world.y));
    }
}

/// Строит пространственный индекс по раскладке.
fn build_index(layout_nodes: &[NodeLayout]) -> SpatialIndex {
    let mut index = SpatialIndex::new(NODE_WIDTH + GAP_X);
    for node in layout_nodes {
        index.insert(node.id, node.rect());
    }
    index
}

/// Цвет темы в цвет egui.
fn color(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Рисует сетку только по видимой области.
fn draw_grid(painter: &egui::Painter, state: &CanvasState, screen: EguiRect, theme: &Theme) {
    let step = theme.metrics.grid_step * state.scale;
    if step < 4.0 {
        return;
    }

    let grid_color = color(theme.palette.grid);
    let origin = state.to_world(screen.min);
    let end = state.to_world(screen.max);

    let start_x = (origin.x / theme.metrics.grid_step).floor() * theme.metrics.grid_step;
    let start_y = (origin.y / theme.metrics.grid_step).floor() * theme.metrics.grid_step;

    let mut x = start_x;
    while x <= end.x {
        let screen_x = state.to_screen(Pos2::new(x, 0.0)).x;
        painter.line_segment(
            [
                Pos2::new(screen_x, screen.min.y),
                Pos2::new(screen_x, screen.max.y),
            ],
            Stroke::new(1.0, grid_color),
        );
        x += theme.metrics.grid_step;
    }

    let mut y = start_y;
    while y <= end.y {
        let screen_y = state.to_screen(Pos2::new(0.0, y)).y;
        painter.line_segment(
            [
                Pos2::new(screen.min.x, screen_y),
                Pos2::new(screen.max.x, screen_y),
            ],
            Stroke::new(1.0, grid_color),
        );
        y += theme.metrics.grid_step;
    }
}

/// Рисует рёбра ортогональными ломаными.
///
/// Рёбра рисуются только между видимыми узлами: стрелка в никуда — это шум.
fn draw_edges(
    painter: &egui::Painter,
    state: &CanvasState,
    layout_nodes: &[NodeLayout],
    graph: &Graph,
    visible: &HashSet<u32>,
    theme: &Theme,
) {
    if !visible.is_empty() {
        let edge_color = color(theme.palette.edge);
        let stroke = Stroke::new(1.5, edge_color);

        let positions: HashMap<u32, &NodeLayout> =
            layout_nodes.iter().map(|node| (node.id, node)).collect();

        for (parent_id, child_id) in &graph.edges {
            if !visible.contains(parent_id) || !visible.contains(child_id) {
                continue;
            }
            let (Some(parent), Some(child)) = (
                positions.get(parent_id).copied(),
                positions.get(child_id).copied(),
            ) else {
                continue;
            };

            let from = state.to_screen(Pos2::new(
                parent.x + parent.width,
                parent.y + parent.height / 2.0,
            ));
            let to = state.to_screen(Pos2::new(child.x, child.y + child.height / 2.0));

            // Ортогональная ломаная: выход вправо, вертикальный участок, вход слева.
            let middle_x = (from.x + to.x) / 2.0;
            painter.line_segment([from, Pos2::new(middle_x, from.y)], stroke);
            painter.line_segment(
                [Pos2::new(middle_x, from.y), Pos2::new(middle_x, to.y)],
                stroke,
            );
            painter.line_segment([Pos2::new(middle_x, to.y), to], stroke);
        }
    }
}

/// Рисует узлы с учётом уровня детализации.
fn draw_nodes(
    painter: &egui::Painter,
    state: &CanvasState,
    layout_nodes: &[NodeLayout],
    graph: &Graph,
    visible: &HashSet<u32>,
    theme: &Theme,
) {
    let detail = lod::detail_for_scale(state.scale);
    let fill = color(theme.palette.surface);
    let accent = color(theme.palette.accent);
    let text_color = color(theme.palette.text);
    let muted_color = color(theme.palette.muted);

    let labels: HashMap<u32, &crate::GraphNode> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();

    for node in layout_nodes {
        if !visible.contains(&node.id) {
            continue;
        }

        let rect = EguiRect::from_min_size(
            state.to_screen(Pos2::new(node.x, node.y)),
            Vec2::new(node.width * state.scale, node.height * state.scale),
        );

        match detail {
            Detail::Points => {
                let radius = lod::point_size(state.scale);
                painter.circle_filled(rect.center(), radius, fill);
                continue;
            }
            Detail::Boxes | Detail::Full => {
                let selected = state.selected == Some(node.id);
                let border = if selected { accent } else { fill };
                let stroke = Stroke::new(if selected { 2.0 } else { 1.0 }, border);

                painter.rect_filled(rect, theme.metrics.corner_radius, fill);
                painter.rect_stroke(
                    rect,
                    theme.metrics.corner_radius,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }

        if detail == Detail::Full {
            let Some(source) = labels.get(&node.id) else {
                continue;
            };
            // Подпись обрезается по рамке узла: длинное имя не должно
            // вылезать на соседей.
            let clipped = painter.with_clip_rect(rect);
            let font_size = lod::label_size(state.scale);

            clipped.text(
                rect.left_top() + Vec2::new(6.0, 4.0),
                Align2::LEFT_TOP,
                &source.label,
                FontId::proportional(font_size),
                text_color,
            );

            if let Some(subtitle) = &source.subtitle {
                clipped.text(
                    rect.left_top() + Vec2::new(6.0, 4.0 + font_size + 2.0),
                    Align2::LEFT_TOP,
                    subtitle,
                    FontId::proportional(font_size * 0.85),
                    muted_color,
                );
            }
        }
    }
}

/// Рисует миникарту в углу.
fn draw_minimap(
    painter: &egui::Painter,
    state: &CanvasState,
    layout_nodes: &[NodeLayout],
    screen: EguiRect,
    theme: &Theme,
) {
    let Some(bounds) = layout::bounds(layout_nodes) else {
        return;
    };

    let size = Vec2::new(160.0, 120.0);
    let mini = EguiRect::from_min_size(screen.right_bottom() - size, size);
    let background = color(theme.palette.surface);
    let node_color = color(theme.palette.muted);
    let accent = color(theme.palette.accent);

    painter.rect_filled(mini, 4.0, background);
    painter.rect_stroke(
        mini,
        4.0,
        Stroke::new(1.0, node_color),
        egui::StrokeKind::Middle,
    );

    let scale_x = mini.width() / bounds.width;
    let scale_y = mini.height() / bounds.height;

    // На больших графах миникарта рисуется точками: иначе она сама станет
    // узким местом.
    let dense = layout_nodes.len() > 1500;

    for node in layout_nodes {
        let position =
            mini.min + Vec2::new((node.x - bounds.x) * scale_x, (node.y - bounds.y) * scale_y);
        if dense {
            painter.circle_filled(position, 1.0, node_color);
        } else {
            let rect = EguiRect::from_min_size(
                position,
                Vec2::new(node.width * scale_x, node.height * scale_y).max(Vec2::new(1.0, 1.0)),
            );
            painter.rect_filled(rect, 1.0, node_color);
        }
    }

    let view = state.viewport(screen);
    let view_rect = EguiRect::from_min_size(
        mini.min + Vec2::new((view.x - bounds.x) * scale_x, (view.y - bounds.y) * scale_y),
        Vec2::new(view.width * scale_x, view.height * scale_y),
    );
    painter.rect_stroke(
        view_rect,
        2.0,
        Stroke::new(1.0, accent),
        egui::StrokeKind::Middle,
    );
}
