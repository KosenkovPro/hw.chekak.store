//! `mcpad-canvas` — бесконечный canvas с графами.
//!
//! Два режима:
//!
//! * **дерево процессов как граф** — узлы-процессы, ортогональные стрелки;
//! * **граф зависимостей модулей** — кто что загрузил.
//!
//! Общего у аналогов нет: это новая функция, а не перенос чужой.
//!
//! Раскладка, отсечение, уровни детализации и экспорт вынесены в модули без
//! egui, поэтому проверяются тестами; `widget` рисует.

pub mod export;
pub mod index;
pub mod layout;
pub mod lod;
pub mod widget;

use std::collections::HashMap;

pub use export::{to_json, to_svg};
pub use index::SpatialIndex;
pub use layout::{GraphNode, NodeLayout, Rect, edges, layout, visible};
pub use lod::Detail;
pub use widget::{CanvasState, show};

/// Связь между двумя узлами графа модулей.
///
/// Тип живёт в `mcpad-core`: и проба, которая перечисляет модули, и canvas
/// должны говорить об одном и том же.
pub use mcpad_core::ModuleLink;

/// Граф для отрисовки: узлы и рёбра.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(u32, u32)>,
}

impl Graph {
    /// Граф дерева процессов.
    pub fn from_processes(snapshot: &mcpad_core::Snapshot) -> Self {
        let nodes: Vec<GraphNode> = snapshot
            .processes
            .iter()
            .map(|process| GraphNode {
                id: process.key.pid,
                label: process.name.clone(),
                parent: process
                    .parent_pid
                    .filter(|parent| *parent != process.key.pid),
                subtitle: subtitle_for(process),
            })
            .collect();
        let edges = layout::edges(&nodes);
        Graph { nodes, edges }
    }

    /// Граф зависимостей модулей.
    ///
    /// Узел — пара «процесс и модуль», поэтому одинаковый модуль в разных
    /// процессах не склеивается.
    pub fn from_module_links(links: &[ModuleLink]) -> Self {
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<(u32, u32)> = Vec::new();
        let mut ids: HashMap<(u32, String), u32> = HashMap::new();
        let mut next_id = 1u32;

        for link in links {
            let parent_key = (link.pid, link.process_label.clone());
            let child_key = (link.pid, link.module_label.clone());

            let parent_id = *ids.entry(parent_key).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                nodes.push(GraphNode {
                    id,
                    label: link.process_label.clone(),
                    parent: None,
                    subtitle: None,
                });
                id
            });

            let child_id = *ids.entry(child_key).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                nodes.push(GraphNode {
                    id,
                    label: link.module_label.clone(),
                    parent: None,
                    subtitle: link.path.clone(),
                });
                id
            });

            if parent_id != child_id {
                edges.push((parent_id, child_id));
            }
        }

        Graph { nodes, edges }
    }

    /// Раскладка графа.
    ///
    /// Глубина считается по рёбрам, а не по родителю: граф модулей — это DAG,
    /// где у узла может быть несколько предков.
    pub fn layout(&self) -> Vec<NodeLayout> {
        let depths = layout::depths_from_edges(&self.edges);
        layout::layout_by_depths(&self.nodes, &depths, 150.0, 44.0, 70.0, 16.0)
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Вторая строка узла процесса: только то, что реально измерено.
///
/// Неизвестное значение не превращается в ноль и не выдумывается — строка
/// просто оказывается короче.
fn subtitle_for(process: &mcpad_core::ProcessInfo) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(cpu) = process.cpu_percent.value {
        parts.push(format!("CPU {cpu:.1}%"));
    }
    if let Some(memory) = process.memory_working_set.value {
        parts.push(format!("{:.0} MiB", memory as f64 / 1_048_576.0));
    }
    if let Some(threads) = process.thread_count.value {
        parts.push(format!("{threads} thr"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// Строит граф дерева процессов из снимка.
pub fn process_graph(snapshot: &mcpad_core::Snapshot) -> Vec<GraphNode> {
    Graph::from_processes(snapshot).nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpad_core::{ProcessInfo, ProcessKey, Snapshot, SystemInfo};

    #[test]
    fn process_graph_keeps_parent_links() {
        let mut parent = ProcessInfo::new(ProcessKey::from_pid(1), "parent");
        parent.parent_pid = None;
        let mut child = ProcessInfo::new(ProcessKey::from_pid(2), "child");
        child.parent_pid = Some(1);

        let snapshot = Snapshot::new(SystemInfo::default(), vec![parent, child]);
        let graph = Graph::from_processes(&snapshot);

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges, vec![(1, 2)]);
        assert_eq!(graph.nodes[1].parent, Some(1));
    }

    #[test]
    fn self_parenting_is_removed() {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(5), "loop");
        process.parent_pid = Some(5);
        let snapshot = Snapshot::new(SystemInfo::default(), vec![process]);
        let graph = Graph::from_processes(&snapshot);
        assert_eq!(graph.nodes[0].parent, None);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn subtitle_omits_unknown_values() {
        let process = ProcessInfo::new(ProcessKey::from_pid(7), "app");
        // Ничего не измерено — второй строки нет вовсе.
        assert!(subtitle_for(&process).is_none());
    }

    #[test]
    fn subtitle_lists_measured_values() {
        let mut process = ProcessInfo::new(ProcessKey::from_pid(7), "app");
        process.cpu_percent = mcpad_core::Counter::measured(12.5, mcpad_core::Source::Derived);
        let subtitle = subtitle_for(&process).expect("есть подпись");
        assert!(subtitle.contains("CPU 12.5%"));
        // Память неизвестна — в подписи её нет.
        assert!(!subtitle.contains("MiB"));
    }

    #[test]
    fn module_links_become_nodes_and_edges() {
        let links = vec![
            ModuleLink::new(10, "app.exe", "core.dll", None),
            ModuleLink::new(10, "app.exe", "net.dll", None),
        ];
        let graph = Graph::from_module_links(&links);

        // Три узла: приложение и два модуля.
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn module_graph_splits_same_module_across_processes() {
        let links = vec![
            ModuleLink::new(10, "a.exe", "core.dll", None),
            ModuleLink::new(11, "b.exe", "core.dll", None),
        ];
        let graph = Graph::from_module_links(&links);
        // Одинаковое имя модуля в разных процессах — разные узлы.
        assert_eq!(graph.nodes.len(), 4);
    }

    #[test]
    fn module_path_becomes_subtitle() {
        let links = vec![ModuleLink::new(
            10,
            "app.exe",
            "core.dll",
            Some("/opt/app/core.dll".to_string()),
        )];
        let graph = Graph::from_module_links(&links);
        let module = graph
            .nodes
            .iter()
            .find(|node| node.label == "core.dll")
            .expect("узел модуля");
        assert_eq!(module.subtitle.as_deref(), Some("/opt/app/core.dll"));
    }

    #[test]
    fn layout_of_module_graph_places_modules_deeper() {
        let links = vec![
            ModuleLink::new(10, "app.exe", "core.dll", None),
            ModuleLink::new(10, "app.exe", "net.dll", None),
        ];
        let graph = Graph::from_module_links(&links);
        let placed = graph.layout();
        let app = placed
            .iter()
            .find(|node| node.id == 1)
            .expect("узел процесса");
        let module = placed
            .iter()
            .find(|node| node.id == 2)
            .expect("узел модуля");
        assert_eq!(app.depth, 0);
        assert_eq!(module.depth, 1);
        assert!(module.x > app.x);
    }
}
