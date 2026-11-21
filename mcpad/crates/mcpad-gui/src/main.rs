//! `mcpad` — окно приложения.
//!
//! Пять представлений: список процессов, дерево, производительность, canvas и
//! возможности платформы. Данные берутся из того же сэмплера, что и у CLI,
//! поэтому числа совпадают.
//!
//! Тема — один JSON, применяемый на лету: перезагрузка файла меняет окно,
//! canvas и терминал без перезапуска приложения.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use mcpad_canvas::{CanvasState, Graph, ModuleLink};
use mcpad_chart::{ChartSpec, plot_cursor, plot_series};
use mcpad_core::{ProcessInfo, ProcessTree, Snapshot};
use mcpad_probe::{Capability, PlatformProbe, Support};
use mcpad_sampler::Sampler;
use mcpad_series::{Sample, Series, TimeSpan, Timeline};
use mcpad_theme::Theme;

mod i18n;
mod perf;
mod theme_ui;

use i18n::{Key, Lang, Strings};
use perf::PerfHistory;

/// Как часто обновляем данные.
const REFRESH: Duration = Duration::from_secs(1);

/// Сколько процессов показывать в списке.
const MAX_ROWS: usize = 500;

/// Активное представление.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Processes,
    Tree,
    Performance,
    Canvas,
    Capabilities,
}

/// Режим canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanvasMode {
    /// Дерево процессов как граф.
    Processes,
    /// Граф зависимостей модулей.
    Modules,
}

/// По чему сортируем список.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Cpu,
    Memory,
    Pid,
    Name,
}

/// Действия, которые выполняются после вьюхи: заимствования изнутри виджетов
/// мешают менять состояние прямо в замыкании.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderAction {
    ToggleLanguage,
    ToggleTheme,
    ReloadTheme,
}

/// Состояние приложения.
struct App {
    sampler: Sampler,
    theme: Theme,
    /// Файл темы: `None` — встроенные тёмная и светлая.
    theme_path: Option<PathBuf>,
    /// Ошибка загрузки темы; тема остаётся прежней.
    theme_error: Option<String>,
    /// Короткое сообщение о последнем действии.
    notice: Option<String>,
    strings: Strings,
    canvas: CanvasState,
    view: View,
    canvas_mode: CanvasMode,
    selected: Option<u32>,
    filter: String,
    sort: SortKey,
    sort_desc: bool,
    frozen: bool,
    frozen_snapshot: Option<Snapshot>,
    collapsed: HashSet<u32>,
    perf: PerfHistory,
    last_refresh: Instant,
    snapshot: Option<Snapshot>,
    error: Option<String>,
    module_links: Vec<ModuleLink>,
    module_error: Option<String>,
}

impl App {
    fn new(theme_path: Option<PathBuf>, lang: Lang) -> Result<Self, mcpad_core::Error> {
        let probe = platform_probe()?;
        let mut theme = Theme::default_dark();
        let mut theme_error = None;

        if let Some(path) = &theme_path {
            match load_theme(path) {
                Ok(loaded) => theme = loaded,
                Err(error) => theme_error = Some(error),
            }
        }

        Ok(Self {
            sampler: Sampler::new(probe),
            theme,
            theme_path,
            theme_error,
            notice: None,
            strings: Strings::new(lang),
            canvas: CanvasState::default(),
            view: View::Processes,
            canvas_mode: CanvasMode::Processes,
            selected: None,
            filter: String::new(),
            sort: SortKey::Cpu,
            sort_desc: true,
            frozen: false,
            frozen_snapshot: None,
            collapsed: HashSet::new(),
            perf: PerfHistory::new(),
            last_refresh: Instant::now(),
            snapshot: None,
            error: None,
            module_links: Vec::new(),
            module_error: None,
        })
    }

    /// Обновляет снимок, если прошло достаточно времени.
    fn refresh(&mut self) {
        if self.frozen {
            return;
        }
        if self.last_refresh.elapsed() < REFRESH {
            return;
        }
        self.last_refresh = Instant::now();

        match self.sampler.refresh() {
            Ok(snapshot) => {
                self.perf.push_snapshot(&snapshot);
                self.snapshot = Some((*snapshot).clone());
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }

        if self.canvas_mode == CanvasMode::Modules {
            self.collect_modules();
        }
    }

    /// Собирает модули выбранного процесса.
    fn collect_modules(&mut self) {
        let Some(pid) = self.module_target() else {
            self.module_links.clear();
            self.module_error = None;
            return;
        };

        match self.sampler.probe().module_links(pid) {
            Ok(links) => {
                self.module_links = links;
                self.module_error = None;
            }
            Err(error) => {
                self.module_links.clear();
                self.module_error = Some(error.to_string());
            }
        }
    }

    /// Процесс, для которого показываются модули: выбранный или самый
    /// нагруженный.
    fn module_target(&self) -> Option<u32> {
        if let Some(pid) = self.selected {
            return Some(pid);
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot
            .processes
            .iter()
            .filter_map(|process| {
                process
                    .cpu_percent
                    .value
                    .map(|value| (process.key.pid, value))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(pid, _)| pid)
    }

    /// Замораживает или размораживает данные.
    fn toggle_freeze(&mut self) {
        self.frozen = !self.frozen;
        if self.frozen {
            self.frozen_snapshot = self.snapshot.clone();
        } else {
            self.frozen_snapshot = None;
        }
    }

    /// Забирает снимок на время отрисовки.
    ///
    /// Снимок уходит из состояния, чтобы вьюхи могли менять выделение, не
    /// сталкиваясь с заимствованиями.
    fn take_snapshot(&mut self) -> Option<Snapshot> {
        if self.frozen {
            self.frozen_snapshot.take()
        } else {
            self.snapshot.take()
        }
    }

    /// Возвращает снимок в состояние.
    fn restore_snapshot(&mut self, snapshot: Option<Snapshot>) {
        if self.frozen {
            self.frozen_snapshot = snapshot;
        } else {
            self.snapshot = snapshot;
        }
    }

    /// Перечитывает файл темы и применяет её.
    fn reload_theme(&mut self, ctx: &egui::Context) {
        let Some(path) = self.theme_path.clone() else {
            return;
        };
        match load_theme(&path) {
            Ok(loaded) => {
                self.theme = loaded;
                self.theme_error = None;
                self.notice = Some(format!(
                    "{}: {}",
                    self.strings.text(Key::ThemeLoaded),
                    self.theme.name
                ));
            }
            Err(error) => {
                self.theme_error = Some(error);
            }
        }
        theme_ui::apply(ctx, &self.theme);
    }

    /// Переключает встроенную тёмную и светлую тему.
    fn toggle_builtin_theme(&mut self, ctx: &egui::Context) {
        self.theme = if self.theme.dark {
            Theme::default_light()
        } else {
            Theme::default_dark()
        };
        self.theme_path = None;
        self.theme_error = None;
        theme_ui::apply(ctx, &self.theme);
    }

    /// Переключает язык интерфейса.
    fn toggle_language(&mut self) {
        self.strings = Strings::new(self.strings.lang.toggle());
    }
}

/// Читает тему из файла.
fn load_theme(path: &PathBuf) -> Result<Theme, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let theme = Theme::from_json(&text).map_err(|error| error.to_string())?;
    theme.validate().map_err(|error| error.to_string())?;
    Ok(theme)
}

/// Проб для текущей платформы.
fn platform_probe() -> Result<Box<dyn PlatformProbe>, mcpad_core::Error> {
    #[cfg(target_os = "windows")]
    {
        mcpad_backends_windows::platform_probe()
    }
    #[cfg(target_os = "linux")]
    {
        mcpad_backends_linux::platform_probe()
    }
    #[cfg(target_os = "macos")]
    {
        mcpad_backends_macos::platform_probe()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(mcpad_core::Error::Unsupported(
            "платформа не поддерживается".to_string(),
        ))
    }
}

impl eframe::App for App {
    /// Обновление данных — вне отрисовки: так монитор продолжает снимать
    /// снимки, даже когда окно скрыто.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh();
        ctx.request_repaint_after(Duration::from_millis(500));

        // F6 — заморозка.
        if ctx.input(|input| input.key_pressed(egui::Key::F6)) {
            self.toggle_freeze();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let strings = self.strings;
        let mut action: Option<HeaderAction> = None;

        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MCPad");
                ui.separator();
                for (view, key) in [
                    (View::Processes, Key::ViewProcesses),
                    (View::Tree, Key::ViewTree),
                    (View::Performance, Key::ViewPerformance),
                    (View::Canvas, Key::ViewCanvas),
                    (View::Capabilities, Key::ViewCapabilities),
                ] {
                    ui.selectable_value(&mut self.view, view, strings.text(key));
                }
                ui.separator();

                // Всё нужное вычисляется заранее: внутри замыкания меняется
                // только локальное действие.
                let summary = summary_text(self.snapshot.as_ref(), strings);
                if !summary.is_empty() {
                    ui.label(summary);
                }

                let language_label = strings.lang.self_name();
                let theme_label = if self.theme.dark {
                    strings.text(Key::Light)
                } else {
                    strings.text(Key::Dark)
                };
                let has_theme_file = self.theme_path.is_some();

                if ui.button(language_label).clicked() {
                    action = Some(HeaderAction::ToggleLanguage);
                }
                ui.label(format!(
                    "{}: {}",
                    strings.text(Key::Language),
                    strings.lang.code()
                ));
                if ui.button(theme_label).clicked() {
                    action = Some(HeaderAction::ToggleTheme);
                }
                ui.label(strings.text(Key::Theme));
                if has_theme_file && ui.button(strings.text(Key::ReloadTheme)).clicked() {
                    action = Some(HeaderAction::ReloadTheme);
                }
            });

            if let Some(error) = &self.error {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 92, 92),
                    format!("{}: {error}", strings.text(Key::ViewProcesses)),
                );
            }
            if let Some(error) = &self.theme_error {
                ui.colored_label(
                    egui::Color32::from_rgb(227, 179, 65),
                    format!("{}: {error}", strings.text(Key::ThemeFailed)),
                );
            }
            if let Some(notice) = &self.notice {
                ui.colored_label(egui::Color32::from_rgb(98, 211, 148), notice);
            }
        });

        egui::Panel::right("details")
            .default_size(300.0)
            .show(ui, |ui| self.show_details(ui));

        egui::CentralPanel::default().show(ui, |ui| match self.view {
            View::Processes => self.show_processes(ui),
            View::Tree => self.show_tree(ui),
            View::Performance => self.show_performance(ui),
            View::Canvas => self.show_canvas(ui),
            View::Capabilities => self.show_capabilities(ui),
        });

        // Действия выполняются после отрисовки: внутри замыкания менять
        // состояние окна нельзя.
        let ctx = ui.ctx().clone();
        match action {
            Some(HeaderAction::ToggleLanguage) => self.toggle_language(),
            Some(HeaderAction::ToggleTheme) => self.toggle_builtin_theme(&ctx),
            Some(HeaderAction::ReloadTheme) => self.reload_theme(&ctx),
            None => {}
        }
    }
}

impl App {
    /// Список процессов с фильтром и сортировкой.
    fn show_processes(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let Some(snapshot) = self.take_snapshot() else {
            ui.label(strings.text(Key::Collecting));
            return;
        };

        let mut toggle_freeze = false;
        ui.horizontal(|ui| {
            ui.label(strings.text(Key::Search));
            ui.text_edit_singleline(&mut self.filter);
            if ui.button(strings.text(Key::Freeze)).clicked() {
                toggle_freeze = true;
            }
            if self.frozen {
                ui.colored_label(
                    egui::Color32::from_rgb(227, 179, 65),
                    strings.text(Key::Frozen),
                );
            }
        });
        if toggle_freeze {
            self.toggle_freeze();
        }

        // Сортировка вынесена из таблицы: вложенные виджеты с общим
        // заимствованием `self` конфликтуют друг с другом.
        egui::ComboBox::from_id_salt("sort")
            .selected_text(sort_label(self.sort, strings))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.sort, SortKey::Cpu, strings.text(Key::ColumnCpu));
                ui.selectable_value(
                    &mut self.sort,
                    SortKey::Memory,
                    strings.text(Key::ColumnMemory),
                );
                ui.selectable_value(&mut self.sort, SortKey::Pid, strings.text(Key::ColumnPid));
                ui.selectable_value(&mut self.sort, SortKey::Name, strings.text(Key::ColumnName));
            });
        ui.checkbox(&mut self.sort_desc, strings.text(Key::Descending));

        let filter = self.filter.trim().to_lowercase();
        let mut rows: Vec<&ProcessInfo> = snapshot
            .processes
            .iter()
            .filter(|process| matches_filter(process, &filter))
            .collect();
        sort_processes(&mut rows, self.sort, self.sort_desc);
        ui.label(format!("{}: {}", strings.text(Key::Total), rows.len()));

        let mut selected_pid: Option<u32> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("process_grid")
                .striped(true)
                .num_columns(6)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(strings.text(Key::ColumnPid));
                    ui.label(strings.text(Key::ColumnName));
                    ui.label(strings.text(Key::ColumnCpu));
                    ui.label(strings.text(Key::ColumnMemory));
                    ui.label(strings.text(Key::ColumnThreads));
                    ui.label(strings.text(Key::ColumnHandles));

                    for process in rows.iter().take(MAX_ROWS) {
                        let pid = process.key.pid;
                        let is_selected = self.selected == Some(pid);
                        if ui.selectable_label(is_selected, pid.to_string()).clicked() {
                            selected_pid = Some(pid);
                        }
                        ui.label(&process.name);
                        ui.label(format_counter(&process.cpu_percent));
                        ui.label(format_mib(&process.memory_working_set));
                        ui.label(format_count(&process.thread_count));
                        ui.label(format_count(&process.handle_count));
                    }
                });
        });

        if let Some(pid) = selected_pid {
            self.selected = Some(pid);
        }

        self.restore_snapshot(Some(snapshot));
    }

    /// Дерево процессов.
    fn show_tree(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let Some(snapshot) = self.take_snapshot() else {
            ui.label(strings.text(Key::Collecting));
            return;
        };

        let tree = ProcessTree::build(&snapshot);
        let names: std::collections::HashMap<u32, String> = snapshot
            .processes
            .iter()
            .map(|process| (process.key.pid, process.name.clone()))
            .collect();

        egui::ScrollArea::vertical().show(ui, |ui| {
            if tree.roots.is_empty() {
                ui.label(strings.text(Key::NoData));
                return;
            }
            for root in &tree.roots {
                self.show_tree_node(ui, &tree, &names, *root, 0);
            }
        });

        self.restore_snapshot(Some(snapshot));
    }

    /// Один узел дерева.
    fn show_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        tree: &ProcessTree,
        names: &std::collections::HashMap<u32, String>,
        pid: u32,
        depth: u32,
    ) {
        // Глубина ограничена: дерево из данных может быть очень глубоким.
        if depth > 64 {
            return;
        }

        let collapsed = self.collapsed.contains(&pid);
        let children: Vec<u32> = tree.children_of(pid).to_vec();
        let name = names
            .get(&pid)
            .cloned()
            .unwrap_or_else(|| format!("pid {pid}"));
        let is_selected = self.selected == Some(pid);

        let mut toggle = false;
        let mut pick = false;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            if children.is_empty() {
                ui.label("•");
            } else {
                let marker = if collapsed { "▶" } else { "▼" };
                if ui.selectable_label(false, marker).clicked() {
                    toggle = true;
                }
            }
            let label = format!("{name}  ({pid})");
            if ui.selectable_label(is_selected, label).clicked() {
                pick = true;
            }
        });

        if toggle {
            if collapsed {
                self.collapsed.remove(&pid);
            } else {
                self.collapsed.insert(pid);
            }
        }
        if pick {
            self.selected = Some(pid);
        }

        if !collapsed {
            for child in children {
                self.show_tree_node(ui, tree, names, child, depth + 1);
            }
        }
    }

    /// Раздел «Производительность»: графики, окно времени, сводка.
    fn show_performance(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let now = self.perf.last_at_unix_ms;

        // Окно времени и курсор.
        let mut cursor_move: Option<bool> = None;
        ui.horizontal(|ui| {
            ui.label(strings.text(Key::Window));
            for span in TimeSpan::all() {
                ui.selectable_value(&mut self.perf.timeline.span, span, strings.span_label(span));
            }
            ui.separator();
            if ui.button(strings.text(Key::CursorLeft)).clicked() {
                cursor_move = Some(false);
            }
            if ui.button(strings.text(Key::CursorRight)).clicked() {
                cursor_move = Some(true);
            }
        });
        if let Some(forward) = cursor_move {
            self.perf.timeline.move_cursor(now, forward);
        }

        let theme = self.theme.clone();
        let accent = egui::Color32::from_rgb(
            theme.palette.accent.r,
            theme.palette.accent.g,
            theme.palette.accent.b,
        );
        let ok =
            egui::Color32::from_rgb(theme.palette.ok.r, theme.palette.ok.g, theme.palette.ok.b);
        let warning = egui::Color32::from_rgb(
            theme.palette.warning.r,
            theme.palette.warning.g,
            theme.palette.warning.b,
        );

        // Графики рисуются отдельной функцией: так история не держится
        // заимствованной во время отрисовки.
        {
            let timeline = &mut self.perf.timeline;
            let system_cpu = &self.perf.system_cpu;
            show_chart(
                ui,
                &mut *timeline,
                system_cpu,
                now,
                ChartRequest {
                    title: strings.text(Key::ChartSystemCpu),
                    unit: "%",
                    color: accent,
                },
                &theme,
            );
            let memory_used = &self.perf.memory_used;
            show_chart(
                ui,
                &mut *timeline,
                memory_used,
                now,
                ChartRequest {
                    title: strings.text(Key::ChartMemory),
                    unit: "%",
                    color: ok,
                },
                &theme,
            );
        }

        // График выбранного процесса.
        let selected_series = self
            .selected
            .and_then(|pid| self.perf.process_series(pid).cloned());
        if let Some(series) = selected_series {
            let pid = self.selected.unwrap_or_default();
            let name = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.process(pid))
                .map(|process| process.name.clone())
                .unwrap_or_else(|| format!("pid {pid}"));
            ui.separator();
            ui.label(format!("{name} ({pid})"));
            show_chart(
                ui,
                &mut self.perf.timeline,
                &series,
                now,
                ChartRequest {
                    title: strings.text(Key::ChartTopCpu),
                    unit: "%",
                    color: warning,
                },
                &theme,
            );
        }

        // Топ процессов по CPU.
        ui.separator();
        ui.label(strings.text(Key::ChartTopCpu));
        let top = self.perf.top_processes(10);
        if top.is_empty() {
            ui.label(strings.text(Key::NoData));
        } else {
            let names: std::collections::HashMap<u32, String> = self
                .snapshot
                .as_ref()
                .map(|snapshot| {
                    snapshot
                        .processes
                        .iter()
                        .map(|process| (process.key.pid, process.name.clone()))
                        .collect()
                })
                .unwrap_or_default();

            egui::Grid::new("top_grid")
                .num_columns(3)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(strings.text(Key::ColumnPid));
                    ui.label(strings.text(Key::ColumnName));
                    ui.label(strings.text(Key::ColumnCpu));
                    for (pid, value) in &top {
                        ui.label(pid.to_string());
                        ui.label(names.get(pid).cloned().unwrap_or_default());
                        ui.label(format!("{value:.1}"));
                    }
                });
        }

        // Сводка по окну: пропуски видны отдельным числом.
        let from = self.perf.timeline.start(now);
        let to = self.perf.timeline.end(now);
        let stats = self.perf.system_cpu.stats(from, to);
        ui.separator();
        ui.label(strings.text(Key::Statistics));
        egui::Grid::new("stats_grid")
            .num_columns(5)
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(strings.text(Key::Minimum));
                ui.label(strings.text(Key::Maximum));
                ui.label(strings.text(Key::Average));
                ui.label(strings.text(Key::Samples));
                ui.label(strings.text(Key::Gaps));

                match stats {
                    Some(stats) => {
                        ui.label(format!("{:.1}", stats.min));
                        ui.label(format!("{:.1}", stats.max));
                        ui.label(format!("{:.1}", stats.avg));
                        ui.label(stats.measured.to_string());
                        if stats.gaps > 0 {
                            ui.colored_label(warning, stats.gaps.to_string());
                        } else {
                            ui.label("0");
                        }
                    }
                    None => {
                        for _ in 0..5 {
                            ui.label(strings.text(Key::NoData));
                        }
                    }
                }
            });
    }

    /// Canvas: дерево процессов или граф модулей.
    fn show_canvas(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;

        let mut mode_changed = false;
        let mut fit = false;
        let mut reset = false;
        let mut export_svg = false;
        let mut export_json = false;

        ui.horizontal(|ui| {
            if ui
                .selectable_value(
                    &mut self.canvas_mode,
                    CanvasMode::Processes,
                    strings.text(Key::ModeProcesses),
                )
                .clicked()
            {
                mode_changed = true;
            }
            if ui
                .selectable_value(
                    &mut self.canvas_mode,
                    CanvasMode::Modules,
                    strings.text(Key::ModeModules),
                )
                .clicked()
            {
                mode_changed = true;
            }
            ui.separator();
            if ui.button(strings.text(Key::ZoomFit)).clicked() {
                fit = true;
            }
            if ui.button(strings.text(Key::ZoomReset)).clicked() {
                reset = true;
            }
            if ui.button(strings.text(Key::ExportSvg)).clicked() {
                export_svg = true;
            }
            if ui.button(strings.text(Key::ExportJson)).clicked() {
                export_json = true;
            }
        });

        if mode_changed {
            self.collect_modules();
        }

        let graph = match self.canvas_mode {
            CanvasMode::Processes => match self.snapshot.as_ref() {
                Some(snapshot) => Graph::from_processes(snapshot),
                None => Graph::default(),
            },
            CanvasMode::Modules => Graph::from_module_links(&self.module_links),
        };

        if let Some(error) = &self.module_error {
            ui.colored_label(
                egui::Color32::from_rgb(227, 179, 65),
                format!("{}: {error}", strings.text(Key::ModulesUnavailable)),
            );
        } else if let Some(pid) = self.module_target() {
            ui.label(format!("{}: {pid}", strings.text(Key::ModulesOf)));
        }

        let theme = self.theme.clone();
        mcpad_canvas::show(ui, &mut self.canvas, &graph, &theme);

        if fit {
            let placed = graph.layout();
            if let Some(bounds) = mcpad_canvas::layout::bounds(&placed) {
                let rect = ui.max_rect();
                self.canvas.zoom_to_fit(&bounds, rect);
            }
        }
        if reset {
            self.canvas.reset();
        }
        if export_svg {
            let svg = mcpad_canvas::to_svg(&graph, &self.theme);
            ui.ctx().copy_text(svg);
            self.notice = Some(strings.text(Key::Copied).to_string());
        }
        if export_json {
            let json = mcpad_canvas::to_json(&graph);
            ui.ctx().copy_text(json);
            self.notice = Some(strings.text(Key::Copied).to_string());
        }
    }

    /// Возможности платформы.
    fn show_capabilities(&self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let probe = self.sampler.probe();
        ui.label(format!(
            "{}: {}",
            strings.text(Key::ViewCapabilities),
            probe.platform()
        ));
        ui.separator();

        egui::Grid::new("capabilities_grid")
            .num_columns(2)
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(strings.text(Key::ViewCapabilities));
                ui.label(strings.text(Key::Source));

                for capability in Capability::all() {
                    ui.label(capability.as_str());
                    let support = probe.support(*capability);
                    let (text, color) = match support {
                        Support::Full => {
                            ("full".to_string(), egui::Color32::from_rgb(98, 211, 148))
                        }
                        Support::Partial => {
                            ("partial".to_string(), egui::Color32::from_rgb(227, 179, 65))
                        }
                        Support::Unsupported(reason) => (
                            format!("unsupported: {reason}"),
                            egui::Color32::from_rgb(141, 153, 166),
                        ),
                    };
                    ui.colored_label(color, text);
                }
            });

        let limitations = probe.limitations();
        if !limitations.is_empty() {
            ui.separator();
            ui.label(strings.text(Key::Limitations));
            for (capability, reason) in limitations {
                ui.label(format!("{}: {reason}", capability.as_str()));
            }
        }
    }

    /// Панель подробностей выбранного процесса.
    fn show_details(&self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let snapshot = if self.frozen {
            self.frozen_snapshot.as_ref()
        } else {
            self.snapshot.as_ref()
        };

        let Some(snapshot) = snapshot else {
            return;
        };

        let Some(pid) = self.selected else {
            ui.label(strings.text(Key::NoSelection));
            return;
        };

        let Some(process) = snapshot.process(pid) else {
            ui.label(strings.text(Key::ProcessGone));
            return;
        };

        ui.heading(&process.name);
        ui.separator();
        ui.label(format!("PID: {}", process.key.pid));
        if let Some(parent) = process.parent_pid {
            ui.label(format!("{}: {parent}", strings.text(Key::Parent)));
        }
        if let Some(user) = &process.user {
            ui.label(format!("{}: {user}", strings.text(Key::ColumnUser)));
        }
        ui.separator();
        ui.label(format!("CPU: {}", format_counter(&process.cpu_percent)));
        ui.label(format!(
            "{}: {} MiB",
            strings.text(Key::ColumnMemory),
            format_mib(&process.memory_working_set)
        ));
        ui.label(format!(
            "{}: {}",
            strings.text(Key::ColumnThreads),
            format_count(&process.thread_count)
        ));
        ui.label(format!(
            "{}: {}",
            strings.text(Key::ColumnHandles),
            format_count(&process.handle_count)
        ));

        // Provenance: откуда значение и насколько ему верить.
        if let Some(source) = process.cpu_percent.source {
            ui.separator();
            ui.label(format!("{}: {source}", strings.text(Key::Source)));
        }
        if let Some(confidence) = process.cpu_percent.confidence {
            ui.label(format!("{}: {confidence}", strings.text(Key::Confidence)));
        }
        if let Some(reason) = process.cpu_percent.unavailable_reason {
            ui.label(format!("{}: {reason}", strings.text(Key::Unknown)));
        }
    }
}

/// Запрос на отрисовку графика: что показать и каким цветом.
///
/// Отдельная структура, а не длинный список аргументов: у функции с семью
/// и более параметрами clippy начинает проситься.
struct ChartRequest<'a> {
    title: &'a str,
    unit: &'a str,
    color: egui::Color32,
}

/// Рисует один график с курсором по оси времени.
fn show_chart(
    ui: &mut egui::Ui,
    timeline: &mut Timeline,
    series: &Series,
    now: u64,
    request: ChartRequest<'_>,
    theme: &Theme,
) {
    let from = timeline.start(now);
    let to = timeline.end(now);

    // Отсчёты копируются: так график не держит заимствование истории.
    let owned: Vec<Sample> = series.window(from, to).into_iter().copied().collect();
    let samples: Vec<&Sample> = owned.iter().collect();

    let spec = ChartSpec::new(request.title, request.unit, request.color);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 140.0),
        egui::Sense::hover(),
    );
    plot_series(ui, rect, &samples, &spec, theme, from, to);

    if let Some(cursor) = timeline.cursor_unix_ms {
        plot_cursor(ui, rect, cursor, from, to, theme);
    }

    // Курсор следует за мышью над графиком.
    if let Some(position) = response.hover_pos() {
        let span = to.saturating_sub(from).max(1) as f64;
        let ratio = ((position.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        timeline.cursor_unix_ms = Some(from + (span * ratio as f64) as u64);
    }
}

/// Сводка в шапке: загрузка CPU и свободная память.
fn summary_text(snapshot: Option<&Snapshot>, strings: Strings) -> String {
    let Some(snapshot) = snapshot else {
        return String::new();
    };
    let cpu = snapshot
        .system
        .cpu_percent
        .value
        .map(|value| format!("CPU {value:.1}%"))
        .unwrap_or_else(|| "CPU —".to_string());
    let memory = snapshot
        .system
        .memory_available
        .value
        .map(|value| format!("{:.1} MiB free", value as f64 / 1_048_576.0))
        .unwrap_or_else(|| format!("{} —", strings.text(Key::ColumnMemory)));
    format!("{cpu} · {memory}")
}

/// Название поля сортировки.
fn sort_label(key: SortKey, strings: Strings) -> &'static str {
    match key {
        SortKey::Cpu => strings.text(Key::ColumnCpu),
        SortKey::Memory => strings.text(Key::ColumnMemory),
        SortKey::Pid => strings.text(Key::ColumnPid),
        SortKey::Name => strings.text(Key::ColumnName),
    }
}

/// Совпадает ли процесс с фильтром.
fn matches_filter(process: &ProcessInfo, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    process.name.to_lowercase().contains(filter) || process.key.pid.to_string().contains(filter)
}

/// Сортировка списка процессов.
fn sort_processes(rows: &mut [&ProcessInfo], key: SortKey, descending: bool) {
    rows.sort_by(|a, b| {
        let ordering = match key {
            SortKey::Cpu => a
                .cpu_percent
                .value
                .unwrap_or(f64::MIN)
                .total_cmp(&b.cpu_percent.value.unwrap_or(f64::MIN)),
            SortKey::Memory => a
                .memory_working_set
                .value
                .unwrap_or(u64::MIN)
                .cmp(&b.memory_working_set.value.unwrap_or(u64::MIN)),
            SortKey::Pid => a.key.pid.cmp(&b.key.pid),
            SortKey::Name => a.name.cmp(&b.name),
        };
        if descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
}

/// Форматирует долю CPU: неизвестное значение — прочерк.
fn format_counter(counter: &mcpad_core::Counter<f64>) -> String {
    match counter.value {
        Some(value) => format!("{value:.1}"),
        None => "—".to_string(),
    }
}

/// Форматирует память в мегабайтах.
fn format_mib(counter: &mcpad_core::Counter<u64>) -> String {
    match counter.value {
        Some(value) => format!("{:.1}", value as f64 / 1_048_576.0),
        None => "—".to_string(),
    }
}

/// Форматирует счётчик штук.
fn format_count(counter: &mcpad_core::Counter<u32>) -> String {
    match counter.value {
        Some(value) => value.to_string(),
        None => "—".to_string(),
    }
}

/// Разбор аргументов командной строки.
fn parse_args() -> (Option<PathBuf>, Lang) {
    let mut theme_path: Option<PathBuf> = None;
    let mut lang = Lang::Ru;

    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--theme" => {
                if let Some(path) = args.next() {
                    theme_path = Some(PathBuf::from(path));
                }
            }
            "--lang" => {
                if let Some(parsed) = args.next().and_then(|code| Lang::from_code(&code)) {
                    lang = parsed;
                }
            }
            _ => {}
        }
    }

    (theme_path, lang)
}

fn print_usage() {
    println!("MCPad — монитор процессов и железа");
    println!();
    println!("Использование: mcpad [опции]");
    println!();
    println!("  --theme <путь>   JSON-тема оформления");
    println!("  --lang <ru|en>   язык интерфейса");
}

fn main() -> eframe::Result<()> {
    let (theme_path, lang) = parse_args();

    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        print_usage();
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("MCPad"),
        ..Default::default()
    };

    eframe::run_native(
        "MCPad",
        options,
        Box::new(move |cc| {
            // Путь копируется: замыкание создателя может быть вызвано как
            // `Fn`, а двигать из него значение нельзя.
            let app = App::new(theme_path.clone(), lang)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?;
            // Тема применяется сразу, до первого кадра.
            theme_ui::apply(&cc.egui_ctx, &app.theme);
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
