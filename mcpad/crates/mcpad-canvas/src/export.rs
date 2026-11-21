//! Экспорт графа canvas: SVG и JSON.
//!
//! PNG отложен: его нужно снимать с offscreen-текстуры egui, а это отдельная
//! задача отрисовки. SVG и JSON делаются без графического контекста, поэтому
//! результат воспроизводим и проверяется тестами.

use std::collections::HashMap;

use crate::Graph;
use crate::layout::{NodeLayout, bounds};

/// Границы графа с запасом.
fn bounds_with_padding(layout: &[NodeLayout], padding: f32) -> (f32, f32, f32, f32) {
    match bounds(layout) {
        Some(bounds) => (
            bounds.x - padding,
            bounds.y - padding,
            bounds.width + padding * 2.0,
            bounds.height + padding * 2.0,
        ),
        None => (0.0, 0.0, 100.0, 100.0),
    }
}

/// Экспорт в SVG.
pub fn to_svg(graph: &Graph, theme: &mcpad_theme::Theme) -> String {
    let layout = graph.layout();
    let (x, y, width, height) = bounds_with_padding(&layout, 20.0);

    let palette = &theme.palette;
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"{x:.0} {y:.0} {width:.0} {height:.0}\">\n"
    ));
    svg.push_str(&format!(
        "<rect x=\"{x:.0}\" y=\"{y:.0}\" width=\"{width:.0}\" height=\"{height:.0}\" fill=\"{}\"/>\n",
        palette.background.to_hex()
    ));

    // Рёбра — ломаными, как на экране.
    let by_id: HashMap<u32, &crate::GraphNode> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();

    for (parent_id, child_id) in &graph.edges {
        let (Some(parent), Some(child)) = (
            layout.iter().find(|node| node.id == *parent_id),
            layout.iter().find(|node| node.id == *child_id),
        ) else {
            continue;
        };
        let from_x = parent.x + parent.width;
        let from_y = parent.y + parent.height / 2.0;
        let to_x = child.x;
        let to_y = child.y + child.height / 2.0;
        let middle = (from_x + to_x) / 2.0;
        svg.push_str(&format!(
            "<polyline points=\"{from_x:.0},{from_y:.0} {middle:.0},{from_y:.0} {middle:.0},{to_y:.0} {to_x:.0},{to_y:.0}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            palette.edge.to_hex()
        ));
    }

    // Узлы.
    for node in &layout {
        let Some(source) = by_id.get(&node.id) else {
            continue;
        };
        svg.push_str(&format!(
            "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"4\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
            node.x,
            node.y,
            node.width,
            node.height,
            palette.surface.to_hex(),
            palette.accent.to_hex()
        ));
        svg.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{}\" font-family=\"monospace\" font-size=\"12\">{}</text>\n",
            node.x + 8.0,
            node.y + 20.0,
            palette.text.to_hex(),
            escape(&source.label)
        ));
        if let Some(subtitle) = &source.subtitle {
            svg.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{}\" font-family=\"monospace\" font-size=\"10\">{}</text>\n",
                node.x + 8.0,
                node.y + 34.0,
                palette.muted.to_hex(),
                escape(subtitle)
            ));
        }
    }

    svg.push_str("</svg>\n");
    svg
}

/// Экспорт в JSON.
pub fn to_json(graph: &Graph) -> String {
    let layout = graph.layout();
    let by_id: HashMap<u32, &crate::GraphNode> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();

    let mut out = String::from("{\n  \"nodes\": [\n");
    for (index, node) in layout.iter().enumerate() {
        let Some(source) = by_id.get(&node.id) else {
            continue;
        };
        let comma = if index + 1 < layout.len() { "," } else { "" };
        let subtitle = match &source.subtitle {
            Some(text) => format!(", \"subtitle\": \"{}\"", escape(text)),
            None => String::new(),
        };
        out.push_str(&format!(
            "    {{\"id\": {}, \"label\": \"{}\"{} , \"x\": {:.0}, \"y\": {:.0}}}{}\n",
            node.id,
            escape(&source.label),
            subtitle,
            node.x,
            node.y,
            comma
        ));
    }
    out.push_str("  ],\n  \"edges\": [\n");
    for (index, (parent, child)) in graph.edges.iter().enumerate() {
        let comma = if index + 1 < graph.edges.len() {
            ","
        } else {
            ""
        };
        out.push_str(&format!(
            "    {{\"from\": {parent}, \"to\": {child}}}{comma}\n"
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// Экранирование для XML/JSON.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::GraphNode;

    fn graph() -> Graph {
        Graph {
            nodes: vec![
                GraphNode::new(1, "root", None),
                GraphNode::new(2, "child", Some(1)),
            ],
            edges: vec![(1, 2)],
        }
    }

    #[test]
    fn svg_contains_nodes_and_edges() {
        let svg = to_svg(&graph(), &mcpad_theme::Theme::default_dark());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<polyline"));
        assert!(svg.contains("root"));
        assert!(svg.contains("child"));
    }

    #[test]
    fn svg_escapes_labels() {
        let mut graph = graph();
        graph.nodes[0].label = "<script>".to_string();
        let svg = to_svg(&graph, &mcpad_theme::Theme::default_dark());
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn json_is_valid() {
        let json = to_json(&graph());
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON валиден");
        assert_eq!(value["nodes"].as_array().expect("узлы").len(), 2);
        assert_eq!(value["edges"].as_array().expect("рёбра").len(), 1);
        assert_eq!(value["nodes"][0]["label"], "root");
    }

    #[test]
    fn json_carries_subtitle_when_known() {
        let mut graph = graph();
        graph.nodes[0].subtitle = Some("CPU 3.0%".to_string());
        let json = to_json(&graph);
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON валиден");
        assert_eq!(value["nodes"][0]["subtitle"], "CPU 3.0%");
        // У второго узла подписи нет — поле не выдумывается.
        assert!(value["nodes"][1].get("subtitle").is_none());
    }

    #[test]
    fn module_graph_exports_edges() {
        let graph =
            Graph::from_module_links(&[crate::ModuleLink::new(10, "app.exe", "core.dll", None)]);
        let json = to_json(&graph);
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON валиден");
        assert_eq!(value["edges"].as_array().expect("рёбра").len(), 1);
        assert_eq!(value["nodes"].as_array().expect("узлы").len(), 2);
    }
}
