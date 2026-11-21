//! Раскладка графа — чистая геометрия без egui.
//!
//! Отдельно от отрисовки, чтобы правила раскладки и отсечения проверялись
//! тестами: те же тесты идут в CI на всех трёх ОС.

use std::collections::{HashMap, HashSet, VecDeque};

/// Узел графа.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: u32,
    pub label: String,
    /// Родитель в дереве; `None` — корень.
    pub parent: Option<u32>,
    /// Дополнительная строка под подписью (метрики процесса, путь к модулю).
    ///
    /// `None` — данных нет. Подпись без второй строки честнее, чем выдуманная.
    pub subtitle: Option<String>,
}

impl GraphNode {
    /// Узел без дополнительной подписи.
    pub fn new(id: u32, label: impl Into<String>, parent: Option<u32>) -> Self {
        Self {
            id,
            label: label.into(),
            parent,
            subtitle: None,
        }
    }
}

/// Прямоугольник в мировых координатах canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Пересекает ли прямоугольник другой.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Положение узла после раскладки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeLayout {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub depth: u32,
}

impl NodeLayout {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Раскладывает дерево по глубине: X — глубина, Y — порядок среди соседей.
pub fn layout(
    nodes: &[GraphNode],
    node_width: f32,
    node_height: f32,
    gap_x: f32,
    gap_y: f32,
) -> Vec<NodeLayout> {
    let mut depths: HashMap<u32, u32> = HashMap::new();
    for node in nodes {
        depths.insert(node.id, depth_of(node, nodes));
    }
    layout_by_depths(nodes, &depths, node_width, node_height, gap_x, gap_y)
}

/// Раскладывает узлы по готовым глубинам.
///
/// Глубина задаёт колонку (X), порядок внутри колонки — Y. Порядок берётся из
/// порядка узлов во входном списке, поэтому раскладка детерминирована.
pub fn layout_by_depths(
    nodes: &[GraphNode],
    depths: &HashMap<u32, u32>,
    node_width: f32,
    node_height: f32,
    gap_x: f32,
    gap_y: f32,
) -> Vec<NodeLayout> {
    let mut counters: HashMap<u32, u32> = HashMap::new();
    let mut result = Vec::with_capacity(nodes.len());

    for node in nodes {
        let depth = depths.get(&node.id).copied().unwrap_or(0);
        let order = counters.entry(depth).or_insert(0);
        let current = *order;
        *order += 1;

        result.push(NodeLayout {
            id: node.id,
            x: depth as f32 * (node_width + gap_x),
            y: current as f32 * (node_height + gap_y),
            width: node_width,
            height: node_height,
            depth,
        });
    }

    result
}

/// Глубины узлов по списку рёбер «родитель — ребёнок».
///
/// Считается самый длинный путь: узел настолько глубок, насколько глубок его
/// самый глубокий предок. Узлы, попавшие в цикл, глубины не получают — они
/// остаются на нулевом уровне, иначе раскладка разошлась бы напрасно.
pub fn depths_from_edges(edges: &[(u32, u32)]) -> HashMap<u32, u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut indegree: HashMap<u32, usize> = HashMap::new();
    let mut nodes: HashSet<u32> = HashSet::new();

    for (parent, child) in edges {
        children.entry(*parent).or_default().push(*child);
        *indegree.entry(*child).or_insert(0) += 1;
        nodes.insert(*parent);
        nodes.insert(*child);
    }

    let mut depth: HashMap<u32, u32> = HashMap::new();
    let mut queue: VecDeque<u32> = VecDeque::new();

    // Без входящих рёбер — корень.
    for node in &nodes {
        if indegree.get(node).copied().unwrap_or(0) == 0 {
            depth.insert(*node, 0);
            queue.push_back(*node);
        }
    }

    // Топологический обход Кана: каждый узел обрабатывается один раз.
    while let Some(node) = queue.pop_front() {
        let current = depth.get(&node).copied().unwrap_or(0);
        for child in children.get(&node).into_iter().flatten() {
            let next = current + 1;
            let entry = depth.entry(*child).or_insert(next);
            if *entry < next {
                *entry = next;
            }
            let count = indegree.entry(*child).or_insert(0);
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                queue.push_back(*child);
            }
        }
    }

    depth
}

/// Глубина узла в дереве.
pub fn depth_of(node: &GraphNode, nodes: &[GraphNode]) -> u32 {
    let mut depth = 0u32;
    let mut current = node.parent;
    // Ограничение на длину цепочки: защита от зацикливания в данных.
    while let Some(parent_id) = current {
        let Some(parent) = nodes.iter().find(|candidate| candidate.id == parent_id) else {
            break;
        };
        depth += 1;
        current = parent.parent;
        // Потолок глубины: защита от зацикливания в данных. Проверка — до
        // следующего шага, поэтому глубина никогда не превышает предел.
        if depth >= 64 {
            break;
        }
    }
    depth
}

/// Рёбра графа: «родитель — ребёнок».
pub fn edges(nodes: &[GraphNode]) -> Vec<(u32, u32)> {
    nodes
        .iter()
        .filter_map(|node| node.parent.map(|parent| (parent, node.id)))
        .collect()
}

/// Оставляет только узлы, попадающие в viewport.
pub fn visible<'a>(layout: &'a [NodeLayout], viewport: &Rect) -> Vec<&'a NodeLayout> {
    layout
        .iter()
        .filter(|node| node.rect().intersects(viewport))
        .collect()
}

/// Общие границы графа — рамка миникарты.
pub fn bounds(layout: &[NodeLayout]) -> Option<Rect> {
    let first = layout.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.rect().right();
    let mut max_y = first.rect().bottom();

    for node in layout.iter().skip(1) {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.rect().right());
        max_y = max_y.max(node.rect().bottom());
    }

    Some(Rect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1.0),
        height: (max_y - min_y).max(1.0),
    })
}

/// Прямоугольник текущего вида внутри рамки миникарты (нормированные 0..1).
pub fn minimap_viewport(bounds: &Rect, view: &Rect) -> Rect {
    let scale_x = 1.0 / bounds.width;
    let scale_y = 1.0 / bounds.height;
    Rect {
        x: ((view.x - bounds.x) * scale_x).clamp(0.0, 1.0),
        y: ((view.y - bounds.y) * scale_y).clamp(0.0, 1.0),
        width: (view.width * scale_x).clamp(0.0, 1.0),
        height: (view.height * scale_y).clamp(0.0, 1.0),
    }
}

/// Узел под точкой canvas.
pub fn node_at(layout: &[NodeLayout], point: (f32, f32)) -> Option<u32> {
    layout
        .iter()
        .rev()
        .find(|node| {
            let rect = node.rect();
            point.0 >= rect.x
                && point.0 <= rect.right()
                && point.1 >= rect.y
                && point.1 <= rect.bottom()
        })
        .map(|node| node.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nodes() -> Vec<GraphNode> {
        vec![
            GraphNode::new(1, "root", None),
            GraphNode::new(2, "child-a", Some(1)),
            GraphNode::new(3, "child-b", Some(1)),
            GraphNode::new(4, "grandchild", Some(2)),
        ]
    }

    #[test]
    fn depth_grows_with_parents() {
        let nodes = sample_nodes();
        assert_eq!(depth_of(&nodes[0], &nodes), 0);
        assert_eq!(depth_of(&nodes[1], &nodes), 1);
        assert_eq!(depth_of(&nodes[3], &nodes), 2);
    }

    #[test]
    fn layout_places_depths_along_x() {
        let layout = layout(&sample_nodes(), 120.0, 40.0, 60.0, 12.0);
        let by_id: Vec<&NodeLayout> = layout.iter().collect();
        assert_eq!(by_id[0].x, 0.0);
        assert_eq!(by_id[1].x, 180.0);
        assert_eq!(by_id[3].x, 360.0);
    }

    #[test]
    fn siblings_get_different_y() {
        let layout = layout(&sample_nodes(), 120.0, 40.0, 60.0, 12.0);
        let a = layout.iter().find(|node| node.id == 2).expect("узел 2");
        let b = layout.iter().find(|node| node.id == 3).expect("узел 3");
        assert_ne!(a.y, b.y);
    }

    #[test]
    fn edges_follow_parents() {
        let edges = edges(&sample_nodes());
        assert_eq!(edges.len(), 3);
        assert!(edges.contains(&(1, 2)));
        assert!(edges.contains(&(2, 4)));
    }

    #[test]
    fn culling_hides_far_nodes() {
        let layout = layout(&sample_nodes(), 120.0, 40.0, 60.0, 12.0);
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 60.0,
        };
        let visible = visible(&layout, &viewport);
        assert!(visible.iter().any(|node| node.id == 1));
        assert!(!visible.iter().any(|node| node.id == 4));
    }

    #[test]
    fn bounds_cover_all_nodes() {
        let layout = layout(&sample_nodes(), 120.0, 40.0, 60.0, 12.0);
        let bounds = bounds(&layout).expect("границы найдены");
        assert!(bounds.width >= 360.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn minimap_viewport_is_normalized() {
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        let view = Rect {
            x: 10.0,
            y: 5.0,
            width: 20.0,
            height: 10.0,
        };
        let mini = minimap_viewport(&bounds, &view);
        assert!((mini.x - 0.1).abs() < 1e-6);
        assert!((mini.width - 0.2).abs() < 1e-6);
        assert!(mini.x + mini.width <= 1.0);
    }

    #[test]
    fn hit_testing_finds_node() {
        let layout = layout(&sample_nodes(), 120.0, 40.0, 60.0, 12.0);
        let target = layout.iter().find(|node| node.id == 1).expect("узел 1");
        assert_eq!(node_at(&layout, (target.x + 5.0, target.y + 5.0)), Some(1));
        assert_eq!(node_at(&layout, (-100.0, -100.0)), None);
    }

    #[test]
    fn cyclic_parents_do_not_hang() {
        let nodes = vec![
            GraphNode::new(1, "a", Some(2)),
            GraphNode::new(2, "b", Some(1)),
        ];
        // Цикл обрывается ограничением длины цепочки.
        assert!(depth_of(&nodes[0], &nodes) <= 64);
    }

    #[test]
    fn depths_from_edges_follow_longest_path() {
        let depths = depths_from_edges(&[(1, 2), (2, 3), (1, 4)]);
        assert_eq!(depths.get(&1), Some(&0));
        assert_eq!(depths.get(&2), Some(&1));
        assert_eq!(depths.get(&3), Some(&2));
        // Узел 4 — прямой ребёнок корня, он не глубже узла 3.
        assert_eq!(depths.get(&4), Some(&1));
    }

    #[test]
    fn depths_from_edges_survive_cycles() {
        let depths = depths_from_edges(&[(1, 2), (2, 1)]);
        // Узлы цикла не попадают в топологический порядок — глубины у них нет.
        assert!(!depths.contains_key(&1));
        assert!(!depths.contains_key(&2));
    }

    #[test]
    fn depths_from_edges_ignore_dangling_edges() {
        let depths = depths_from_edges(&[(7, 8)]);
        assert_eq!(depths.get(&7), Some(&0));
        assert_eq!(depths.get(&8), Some(&1));
    }

    #[test]
    fn layout_by_depths_stacks_column() {
        let nodes = vec![
            GraphNode::new(1, "a", None),
            GraphNode::new(2, "b", None),
            GraphNode::new(3, "c", None),
        ];
        let mut depths = HashMap::new();
        depths.insert(1, 0);
        depths.insert(2, 0);
        depths.insert(3, 1);
        let placed = layout_by_depths(&nodes, &depths, 100.0, 40.0, 20.0, 10.0);
        assert_eq!(placed[0].x, 0.0);
        assert_eq!(placed[1].x, 0.0);
        assert_ne!(placed[0].y, placed[1].y);
        assert_eq!(placed[2].x, 120.0);
    }

    #[test]
    fn unknown_depth_falls_back_to_zero() {
        let nodes = vec![GraphNode::new(1, "a", None)];
        let placed = layout_by_depths(&nodes, &HashMap::new(), 100.0, 40.0, 20.0, 10.0);
        assert_eq!(placed[0].depth, 0);
        assert_eq!(placed[0].x, 0.0);
    }
}
