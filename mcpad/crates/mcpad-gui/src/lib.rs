//! `mcpad-gui` — окно монитора процессов.
//!
//! Бинарь `mcpad-gui` — тонкая обёртка над этой библиотекой: разбор
//! аргументов и запуск окна. Всё остальное живёт здесь, чтобы логика окна
//! была доступна тестам, а не только запускалась как программа.
//!
//! Раскладка повторяет Диспетчер задач Windows 11: слева — сворачиваемая
//! навигация, сверху — командная панель с поиском, снизу — строка состояния,
//! справа — сведения о выбранном процессе. В отличие от системного
//! диспетчера, здесь те же приёмы применены к единому сэмплеру: числа в окне и
//! в командной строке считаются по одному правилу и совпадают.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use mcpad_canvas::{CanvasState, Graph, ModuleLink};
use mcpad_chart::{ChartSpec, plot_cursor, plot_series};
use mcpad_core::{Counter, Priority, ProcessDetails, ProcessInfo, ProcessTree, Snapshot};
use mcpad_probe::{Capability, PlatformProbe, Support};
use mcpad_sampler::Sampler;
use mcpad_series::{Sample, Series, TimeSpan, Timeline};
use mcpad_theme::Theme;

pub mod columns;
pub mod details;
pub mod export;
pub mod filter;
pub mod group;
pub mod heat;
pub mod i18n;
pub mod perf;
pub mod shell;
pub mod table;
pub mod theme_ui;
pub mod tree;
pub mod widgets;

pub use i18n::{Key, Lang, Strings};
pub use perf::PerfHistory;
pub use shell::{Commands, NavState, View};

/// Как часто обновляем данные.
const REFRESH: Duration = Duration::from_secs(1);

/// Сколько процессов показывать в списке.
const MAX_ROWS: usize = 500;

/// Сколько процессов показывать в топе раздела «Производительность».
const TOP_ROWS: usize = 8;

/// Ширина панели сведений.
const DETAILS_WIDTH: f32 = 320.0;

/// Гигабайт в байтах.
const GIB: f64 = 1_073_741_824.0;

/// Режим canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanvasMode {
    /// Дерево процессов как граф.
    Processes,
    /// Граф зависимостей модулей.
    Modules,
}

/// Состояние приложения.
pub struct App {
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
    nav: NavState,
    table: table::TableState,
    /// Состояние таблицы топа процессов в разделе «Производительность».
    perf_table: table::TableState,
    canvas_mode: CanvasMode,
    selected: Option<u32>,
    filter: String,
    frozen: bool,
    frozen_snapshot: Option<Snapshot>,
    collapsed: HashSet<u32>,
    perf: PerfHistory,
    last_refresh: Instant,
    snapshot: Option<Snapshot>,
    error: Option<String>,
    module_links: Vec<ModuleLink>,
    module_error: Option<String>,
    /// Подробности выбранного процесса — по запросу, а не в горячем пути.
    details: Option<ProcessDetails>,
    /// Ошибка запроса подробностей.
    details_error: Option<String>,
    /// Для какого PID запрошены подробности.
    details_pid: Option<u32>,
    /// Модули выбранного процесса — для панели сведений.
    detail_modules: Vec<ModuleLink>,
    /// Ошибка перечисления модулей.
    detail_modules_error: Option<String>,
    /// Для какого PID собраны модули.
    detail_modules_pid: Option<u32>,
    /// Показывать ли панель сведений.
    details_visible: bool,
    /// Компактные ли строки таблицы.
    compact: bool,
    /// Передать ли фокус в поле поиска.
    focus_search: bool,
    /// Как группировать процессы в списке.
    group_by: group::GroupBy,
    /// Профиль набора столбцов.
    profile: columns::Profile,
    /// Быстрые фильтры, включённые кнопками.
    quick_filters: HashSet<filter::QuickFilter>,
    /// Скрывать ли системные процессы.
    hide_system: bool,
    /// Интервал обновления данных.
    refresh_interval: Duration,
    /// PID процессов в порядке показа — для навигации стрелками.
    visible_pids: Vec<u32>,
    /// Приоритет, который будет назначен выбранному процессу.
    ///
    /// Это целевой класс, а не прочитанный: текущий класс приоритета
    /// процессов мы не собираем, поэтому показывать его нечем.
    priority: Priority,
    /// PID процесса, для которого ждёт подтверждения завершения.
    confirm_terminate: Option<u32>,
    /// Режим поиска: обычный текст или регулярное выражение.
    filter_mode: filter::Mode,
}

impl App {
    /// Окно для текущей платформы.
    pub fn new(
        theme_path: Option<PathBuf>,
        lang: Lang,
        view: View,
        group_by: group::GroupBy,
    ) -> Result<Self, mcpad_core::Error> {
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
            nav: NavState {
                view,
                collapsed: false,
            },
            table: table::TableState::default(),
            perf_table: table::TableState {
                sort: columns::Column::Cpu,
                descending: true,
                visible: vec![
                    columns::Column::Pid,
                    columns::Column::Name,
                    columns::Column::Cpu,
                    columns::Column::History,
                ],
            },
            canvas_mode: CanvasMode::Processes,
            selected: None,
            filter: String::new(),
            frozen: false,
            frozen_snapshot: None,
            collapsed: HashSet::new(),
            perf: PerfHistory::new(),
            last_refresh: Instant::now(),
            snapshot: None,
            error: None,
            module_links: Vec::new(),
            module_error: None,
            details: None,
            details_error: None,
            details_pid: None,
            detail_modules: Vec::new(),
            detail_modules_error: None,
            detail_modules_pid: None,
            details_visible: true,
            compact: false,
            focus_search: false,
            group_by,
            profile: columns::Profile::Basic,
            quick_filters: HashSet::new(),
            hide_system: false,
            refresh_interval: REFRESH,
            visible_pids: Vec::new(),
            priority: Priority::Normal,
            confirm_terminate: None,
            filter_mode: filter::Mode::Text,
        })
    }

    /// Обновляет снимок, если прошло достаточно времени.
    fn refresh(&mut self) {
        if self.frozen {
            return;
        }
        if self.last_refresh.elapsed() < self.refresh_interval {
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

    /// Запрашивает подробности выбранного процесса.
    ///
    /// Это дорогой путь: путь к образу, командная строка и владелец требуют
    /// открыть процесс. Поэтому сведения запрашиваются только для выбранного
    /// процесса и перезапрашиваются при смене выбора.
    fn refresh_details(&mut self) {
        let Some(pid) = self.selected else {
            self.details = None;
            self.details_error = None;
            self.details_pid = None;
            return;
        };
        if self.details_pid == Some(pid) {
            return;
        }
        self.details_pid = Some(pid);
        match self.sampler.probe().process_details(pid) {
            Ok(details) => {
                self.details = Some(details);
                self.details_error = None;
            }
            Err(error) => {
                self.details = None;
                self.details_error = Some(error.to_string());
            }
        }
    }

    /// Собирает модули выбранного процесса.
    ///
    /// Перечисление модулей дорого, поэтому оно идёт один раз на выбор
    /// процесса, а не каждый кадр.
    fn refresh_modules(&mut self) {
        let Some(pid) = self.selected else {
            self.detail_modules.clear();
            self.detail_modules_error = None;
            self.detail_modules_pid = None;
            return;
        };
        if self.detail_modules_pid == Some(pid) {
            return;
        }
        self.detail_modules_pid = Some(pid);

        match self.sampler.probe().module_links(pid) {
            Ok(links) => {
                self.detail_modules = links;
                self.detail_modules_error = None;
            }
            Err(error) => {
                self.detail_modules.clear();
                self.detail_modules_error = Some(error.to_string());
            }
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

    /// Завершает процесс.
    ///
    /// Действие разрушительное, поэтому подтверждение двушаговое: первое
    /// нажатие только спрашивает, второе — завершает. Ошибка показывается
    /// текстом: «нет прав» и «процесса нет» — это разные вещи.
    fn terminate_process(&mut self, pid: u32) {
        if self.confirm_terminate != Some(pid) {
            self.confirm_terminate = Some(pid);
            return;
        }
        self.confirm_terminate = None;

        match self.sampler.probe().terminate_process(pid) {
            Ok(()) => {
                self.notice = Some(self.strings.text(Key::Terminated).to_string());
                if self.selected == Some(pid) {
                    self.selected = None;
                }
            }
            Err(error) => {
                self.notice = Some(format!(
                    "{}: {error}",
                    self.strings.text(Key::TerminateFailed)
                ));
            }
        }
    }

    /// Назначает процессу приоритет.
    fn apply_priority(&mut self, pid: u32, priority: Priority) {
        self.priority = priority;
        match self.sampler.probe().set_priority(pid, priority) {
            Ok(()) => {
                self.notice = Some(self.strings.text(Key::PriorityChanged).to_string());
            }
            Err(error) => {
                self.notice = Some(format!("{}: {error}", self.strings.text(Key::Priority)));
            }
        }
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

        let mut focus_search = false;
        let mut clear_search = false;
        let mut view: Option<View> = None;

        ctx.input(|input| {
            if input.key_pressed(egui::Key::F6) {
                self.toggle_freeze();
            }
            if input.modifiers.ctrl && input.key_pressed(egui::Key::F) {
                focus_search = true;
            }
            if input.key_pressed(egui::Key::Escape) {
                clear_search = true;
            }
            if input.modifiers.ctrl {
                for (index, candidate) in View::all().iter().enumerate() {
                    let digit = match index {
                        0 => egui::Key::Num1,
                        1 => egui::Key::Num2,
                        2 => egui::Key::Num3,
                        3 => egui::Key::Num4,
                        _ => egui::Key::Num5,
                    };
                    if input.key_pressed(digit) {
                        view = Some(*candidate);
                    }
                }
            }
        });

        if focus_search {
            self.focus_search = true;
        }
        if clear_search {
            self.filter.clear();
        }
        if let Some(view) = view {
            self.nav.view = view;
        }

        // Навигация по списку процессов стрелками — как в Диспетчере задач и
        // в RustWTM. Пока фокус в поле поиска, стрелки остаются стрелками.
        let widget_focused = ctx.memory(|memory| memory.focused()).is_some();
        if self.nav.view == View::Processes && !widget_focused {
            let mut step: Option<isize> = None;
            ctx.input(|input| {
                if input.key_pressed(egui::Key::ArrowDown) {
                    step = Some(1);
                } else if input.key_pressed(egui::Key::ArrowUp) {
                    step = Some(-1);
                } else if input.key_pressed(egui::Key::PageDown) {
                    step = Some(10);
                } else if input.key_pressed(egui::Key::PageUp) {
                    step = Some(-10);
                } else if input.key_pressed(egui::Key::Home) {
                    step = Some(isize::MIN);
                } else if input.key_pressed(egui::Key::End) {
                    step = Some(isize::MAX);
                }
            });
            if let Some(step) = step {
                self.move_selection(step);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let strings = self.strings;
        let theme = self.theme.clone();
        let frozen = self.frozen;
        let details_visible = self.details_visible;
        let has_theme_file = self.theme_path.is_some();

        let mut commands = Commands::default();
        let mut focus_search = self.focus_search;
        let mut filter = std::mem::take(&mut self.filter);
        let mut filter_mode = self.filter_mode;
        let mut nav = self.nav;

        // Командная панель.
        egui::Panel::top("command")
            .frame(
                egui::Frame::new()
                    .fill(heat::rgb_to_color(theme.palette.surface))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                shell::command_bar(
                    ui,
                    &theme,
                    strings,
                    &mut filter,
                    &mut filter_mode,
                    frozen,
                    details_visible,
                    has_theme_file,
                    &mut focus_search,
                    &mut commands,
                );
            });

        // Строка состояния.
        let snapshot_time = self
            .snapshot
            .as_ref()
            .map(|snapshot| format_clock(snapshot.taken_at_unix_ms));
        let process_count = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.processes.len());
        let platform = self
            .snapshot
            .as_ref()
            .map(|snapshot| format!("{} · {}", snapshot.system.os_name, snapshot.system.arch))
            .unwrap_or_default();
        let mut interval = self.refresh_interval;
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(heat::rgb_to_color(theme.palette.surface))
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                shell::status_bar(
                    ui,
                    &theme,
                    strings,
                    snapshot_time,
                    process_count,
                    &mut interval,
                    frozen,
                    &platform,
                );
            });
        self.refresh_interval = interval;

        // Боковая навигация.
        let nav_width = if nav.collapsed {
            shell::SIDEBAR_COLLAPSED
        } else {
            shell::SIDEBAR_WIDTH
        };
        let subtitle = platform.clone();
        egui::Panel::left("nav")
            .exact_size(nav_width)
            .frame(
                egui::Frame::new()
                    .fill(heat::rgb_to_color(theme.palette.surface))
                    .inner_margin(egui::Margin::symmetric(4, 8)),
            )
            .show(ui, |ui| {
                shell::sidebar(ui, &theme, strings, &mut nav, &subtitle);
            });

        // Панель сведений о выбранном процессе.
        if details_visible {
            egui::Panel::right("details")
                .exact_size(DETAILS_WIDTH)
                .frame(
                    egui::Frame::new()
                        .fill(heat::rgb_to_color(theme.palette.surface))
                        .inner_margin(egui::Margin::symmetric(10, 10)),
                )
                .show(ui, |ui| {
                    let details = self.details_for_panel();
                    details::details_panel(
                        ui,
                        &theme,
                        strings,
                        details,
                        &self.detail_modules,
                        self.detail_modules_error.as_deref(),
                    );
                });
        }

        // Рабочая область.
        egui::CentralPanel::default().show(ui, |ui| match nav.view {
            View::Processes => self.show_processes(ui, &filter),
            View::Tree => self.show_tree(ui),
            View::Performance => self.show_performance(ui),
            View::Canvas => self.show_canvas(ui),
            View::Capabilities => self.show_capabilities(ui),
        });

        // Команды выполняются после отрисовки: внутри замыкания менять
        // состояние окна нельзя.
        let ctx = ui.ctx().clone();
        self.filter = filter;
        self.filter_mode = filter_mode;
        self.focus_search = focus_search;
        self.nav = nav;
        self.refresh_details();
        self.refresh_modules();

        if commands.toggle_freeze {
            self.toggle_freeze();
        }
        if commands.toggle_theme {
            self.toggle_builtin_theme(&ctx);
        }
        if commands.toggle_language {
            self.toggle_language();
        }
        if commands.toggle_details {
            self.details_visible = !self.details_visible;
        }
        if commands.reload_theme {
            self.reload_theme(&ctx);
        }
    }
}

impl App {
    /// Перемещает выделение по видимому списку процессов.
    ///
    /// Шаг положительный — вниз, отрицательный — вверх. `Home` и `End`
    /// передаются как предельные значения.
    fn move_selection(&mut self, step: isize) {
        if self.visible_pids.is_empty() {
            return;
        }
        let last = self.visible_pids.len() as isize - 1;
        let current = self.selected.and_then(|pid| {
            self.visible_pids
                .iter()
                .position(|candidate| *candidate == pid)
        });
        let index = match current {
            Some(index) => (index as isize + step).clamp(0, last),
            None => {
                if step < 0 {
                    last
                } else {
                    0
                }
            }
        };
        self.selected = self.visible_pids.get(index as usize).copied();
    }

    /// Сведения для панели: снимок процесса плюс запрошенные подробности.
    fn details_for_panel(&self) -> details::Details {
        let snapshot = if self.frozen {
            self.frozen_snapshot.as_ref()
        } else {
            self.snapshot.as_ref()
        };
        let Some(snapshot) = snapshot else {
            return details::Details::Empty;
        };
        let Some(pid) = self.selected else {
            return details::Details::Empty;
        };
        let Some(process) = snapshot.process(pid) else {
            return details::Details::Gone;
        };

        // Подробности подставляются в копию: путь, командная строка и владелец
        // приходят отдельным запросом и в снимке их нет.
        let mut merged = process.clone();
        if let Some(details) = &self.details {
            merged.exe_path = details.exe_path.clone();
            merged.command_line = details.command_line.clone();
            merged.user = details.user.clone();
        }
        details::Details::Process(Box::new(merged))
    }

    /// Список процессов: карточки сводки, инструменты и таблица.
    fn show_processes(&mut self, ui: &mut egui::Ui, filter_text: &str) {
        let strings = self.strings;
        let theme = self.theme.clone();
        let muted = heat::rgb_to_color(theme.palette.muted);
        let Some(snapshot) = self.take_snapshot() else {
            ui.label(strings.text(Key::Collecting));
            self.restore_snapshot(None);
            return;
        };

        self.show_summary_cards(ui, &snapshot);

        let mut export: Option<export::Format> = None;

        // Инструменты: профиль столбцов, плотность, группировка.
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("profile")
                .selected_text(egui::RichText::new(profile_label(strings, self.profile)).size(12.0))
                .show_ui(ui, |ui| {
                    for profile in columns::Profile::all() {
                        if ui
                            .selectable_label(
                                self.profile == profile,
                                profile_label(strings, profile),
                            )
                            .clicked()
                        {
                            self.profile = profile;
                            self.table.visible = profile.columns();
                        }
                    }
                });
            ui.separator();
            table::column_chooser(ui, strings, &mut self.table);
            ui.separator();
            if ui
                .selectable_label(!self.compact, strings.text(Key::Comfortable))
                .clicked()
            {
                self.compact = false;
            }
            if ui
                .selectable_label(self.compact, strings.text(Key::Compact))
                .clicked()
            {
                self.compact = true;
            }
            ui.separator();
            egui::ComboBox::from_id_salt("group_by")
                .selected_text(egui::RichText::new(group_label(strings, self.group_by)).size(12.0))
                .show_ui(ui, |ui| {
                    for option in group::GroupBy::all() {
                        if ui
                            .selectable_label(self.group_by == option, group_label(strings, option))
                            .clicked()
                        {
                            self.group_by = option;
                        }
                    }
                });
            ui.separator();
            if ui
                .selectable_label(self.hide_system, strings.text(Key::HideSystem))
                .clicked()
            {
                self.hide_system = !self.hide_system;
            }
        });
        ui.add_space(4.0);

        // Фильтр: выражение из строки поиска, быстрые фильтры и скрытие
        // системных процессов.
        let parsed = filter::Filter::parse_with_mode(filter_text, self.filter_mode);
        if let Some(error) = parsed.error() {
            ui.colored_label(
                heat::rgb_to_color(theme.palette.critical),
                format!("{}: {error}", strings.text(Key::InvalidFilter)),
            );
        }
        let mut rows: Vec<&ProcessInfo> = snapshot
            .processes
            .iter()
            .filter(|process| parsed.matches(process))
            .collect();

        // Карта процессов — полная, по снимку. Родители нужны и тем, кто
        // отфильтрован из списка: иначе «скрыть системные» перестаёт видеть
        // системное дерево, а группа называется «pid N».
        let known: HashMap<u32, &ProcessInfo> = snapshot
            .processes
            .iter()
            .map(|process| (process.key.pid, process))
            .collect();

        if self.hide_system {
            rows.retain(|process| !group::is_system_process(process, &known));
        }
        for quick in &self.quick_filters {
            rows.retain(|process| quick.matches(process));
        }

        table::sort_rows(&mut rows, &self.table);
        rows.truncate(MAX_ROWS);

        // Быстрые фильтры — кнопки-чипы, как в RustWTM.
        ui.horizontal(|ui| {
            for quick in filter::QuickFilter::all() {
                let active = self.quick_filters.contains(&quick);
                let label = match quick {
                    filter::QuickFilter::HighCpu => strings.text(Key::FilterHighCpu),
                    filter::QuickFilter::HighMemory => strings.text(Key::FilterHighMemory),
                };
                if ui
                    .selectable_label(active, egui::RichText::new(label).size(11.0))
                    .clicked()
                {
                    if active {
                        self.quick_filters.remove(&quick);
                    } else {
                        self.quick_filters.insert(quick);
                    }
                }
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!(
                    "{}: {} / {}",
                    strings.text(Key::Matched),
                    rows.len(),
                    snapshot.processes.len()
                ))
                .size(11.0)
                .color(muted),
            );

            // Экспорт — справа от быстрых фильтров: так кнопки не заезжают под
            // панель сведений, а счётчик найденных остаётся рядом с ними.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                if ui
                    .button(egui::RichText::new(strings.text(Key::ExportJson)).size(12.0))
                    .clicked()
                {
                    export = Some(export::Format::Json);
                }
                if ui
                    .button(egui::RichText::new(strings.text(Key::ExportCsv)).size(12.0))
                    .clicked()
                {
                    export = Some(export::Format::Csv);
                }
            });
        });
        ui.add_space(4.0);

        // Порядок показа нужен для навигации стрелками.
        self.visible_pids = rows.iter().map(|process| process.key.pid).collect();

        // Мини-графики собираются заранее: история не должна быть заимствована
        // во время отрисовки строк.
        let sparklines: HashMap<u32, Vec<Option<f64>>> = rows
            .iter()
            .filter_map(|process| {
                let values = self
                    .perf
                    .recent_values(process.key.pid, table::SPARK_SAMPLES);
                if values.is_empty() {
                    None
                } else {
                    Some((process.key.pid, values))
                }
            })
            .collect();

        let columns = self.table.visible.clone();
        let layout = table::Layout::new(&columns);
        let memory_total = snapshot.system.memory_total.value;

        // Группы: заголовок с итогами и процессы под ним.
        let groups = group::group_with(&rows, self.group_by, &known);
        let mut table_rows: Vec<table::TableRow> = Vec::with_capacity(rows.len() + groups.len());
        for group in &groups {
            if self.group_by != group::GroupBy::None {
                table_rows.push(table::TableRow::Group(table::GroupHeader {
                    label: group.label.clone(),
                    count: group.rows.len(),
                    cpu: group.cpu(),
                    memory: group.memory(),
                }));
            }
            for process in &group.rows {
                table_rows.push(table::TableRow::Process(process));
            }
        }

        // Панель действий над выбранным процессом — как в Task Explorer и
        // System Informer. Она стоит над таблицей: иначе длинный список
        // прижимает её к нижнему краю и она не помещается в окно.
        if let Some(pid) = self.selected {
            let process = snapshot.process(pid);
            let name = process
                .map(|process| process.name.clone())
                .unwrap_or_default();
            let path = self
                .details
                .as_ref()
                .and_then(|details| details.exe_path.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{name} ({pid})"))
                        .size(11.0)
                        .color(muted),
                );
                if ui
                    .button(egui::RichText::new(strings.text(Key::CopyPid)).size(11.0))
                    .clicked()
                {
                    ui.ctx().copy_text(pid.to_string());
                    self.notice = Some(strings.text(Key::Copied).to_string());
                }
                if ui
                    .button(egui::RichText::new(strings.text(Key::CopyName)).size(11.0))
                    .clicked()
                {
                    ui.ctx().copy_text(name.clone());
                    self.notice = Some(strings.text(Key::Copied).to_string());
                }
                if !path.is_empty()
                    && ui
                        .button(egui::RichText::new(strings.text(Key::CopyPath)).size(11.0))
                        .clicked()
                {
                    ui.ctx().copy_text(path.clone());
                    self.notice = Some(strings.text(Key::Copied).to_string());
                }

                ui.separator();

                // Приоритет: как «Set priority» в Диспетчере задач.
                let mut priority = self.priority;
                egui::ComboBox::from_id_salt("priority")
                    .selected_text(
                        egui::RichText::new(priority_label(strings, priority)).size(11.0),
                    )
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for option in Priority::ALL {
                            ui.selectable_value(
                                &mut priority,
                                option,
                                priority_label(strings, option),
                            );
                        }
                    });
                if priority != self.priority {
                    self.apply_priority(pid, priority);
                }

                ui.separator();

                // Завершение процесса: «Снять задачу», как в Task Explorer.
                let armed = self.confirm_terminate == Some(pid);
                let protected = process.is_some_and(|process| process.is_protected == Some(true));
                let label = if armed {
                    strings.text(Key::TerminateConfirm)
                } else {
                    strings.text(Key::Terminate)
                };
                let color = if armed {
                    heat::rgb_to_color(theme.palette.critical)
                } else {
                    heat::rgb_to_color(theme.palette.text)
                };
                let button = egui::Button::new(egui::RichText::new(label).size(11.0).color(color));
                let response = ui.add_enabled(!protected, button);
                if protected {
                    response.on_hover_text(strings.text(Key::ProtectedProcess));
                } else if response.clicked() {
                    self.terminate_process(pid);
                }
            });
            ui.add_space(4.0);
        }

        let mut picked: Option<u32> = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            table::header(ui, &theme, strings, &mut self.table, &layout);
            egui::ScrollArea::vertical()
                .id_salt("process_rows")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(pid) = table::rows(
                        ui,
                        &theme,
                        strings,
                        &columns,
                        &layout,
                        &table_rows,
                        memory_total,
                        &sparklines,
                        self.selected,
                        self.compact,
                    ) {
                        picked = Some(pid);
                    }
                });
        });

        if let Some(pid) = picked {
            self.selected = Some(pid);
        }

        // Экспорт: видимая таблица уходит в буфер обмена.
        if let Some(format) = export {
            let text = match format {
                export::Format::Csv => export::to_csv(&columns, &rows, strings),
                export::Format::Json => export::to_json(&columns, &rows, strings),
            };
            ui.ctx().copy_text(text);
            self.notice = Some(strings.text(Key::Copied).to_string());
        }

        self.restore_snapshot(Some(snapshot));
    }

    /// Карточки сводки: загрузка, память, количество процессов и ресурсов.
    fn show_summary_cards(&self, ui: &mut egui::Ui, snapshot: &Snapshot) {
        let theme = &self.theme;
        let strings = self.strings;
        let accent = heat::rgb_to_color(theme.palette.accent);
        let ok = heat::rgb_to_color(theme.palette.ok);
        let muted = heat::rgb_to_color(theme.palette.muted);

        let cpu = snapshot.system.cpu_percent.value;
        let cpu_text = match cpu {
            Some(value) => format!("{value:.1} %"),
            None => "—".to_string(),
        };

        let total = snapshot.system.memory_total.value;
        let available = snapshot.system.memory_available.value;
        let used = match (total, available) {
            (Some(total), Some(available)) if total > 0 => Some(total - available),
            _ => None,
        };
        let memory_text = match (used, total) {
            (Some(used), Some(total)) => {
                format!("{:.1} / {:.1} ГБ", used as f64 / GIB, total as f64 / GIB)
            }
            _ => "—".to_string(),
        };
        let memory_ratio = match (used, total) {
            (Some(used), Some(total)) if total > 0 => Some(used as f64 / total as f64),
            _ => None,
        };

        let (threads, threads_unknown) =
            sum_counters(snapshot.processes.iter().map(|p| &p.thread_count));
        let (handles, handles_unknown) =
            sum_counters(snapshot.processes.iter().map(|p| &p.handle_count));

        let processes_text = snapshot.processes.len().to_string();
        let threads_text = if threads_unknown > 0 {
            format!("{threads} + ?")
        } else {
            threads.to_string()
        };
        let handles_text = if handles_unknown > 0 {
            format!("{handles} + ?")
        } else {
            handles.to_string()
        };

        ui.horizontal(|ui| {
            let width = ((ui.available_width() - 32.0) / 5.0).max(120.0);
            for (label, value, ratio, color) in [
                (
                    strings.text(Key::CpuLoad),
                    cpu_text.as_str(),
                    cpu.map(|value| value / 100.0),
                    accent,
                ),
                (
                    strings.text(Key::MemoryUsed),
                    memory_text.as_str(),
                    memory_ratio,
                    ok,
                ),
                (
                    strings.text(Key::Total),
                    processes_text.as_str(),
                    None,
                    muted,
                ),
                (
                    strings.text(Key::TotalThreads),
                    threads_text.as_str(),
                    None,
                    muted,
                ),
                (
                    strings.text(Key::TotalHandles),
                    handles_text.as_str(),
                    None,
                    muted,
                ),
            ] {
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    widgets::stat_card(
                        ui,
                        theme,
                        widgets::StatCard {
                            label,
                            value,
                            ratio,
                            color,
                        },
                    );
                });
            }
        });
        ui.add_space(4.0);

        if let Some(error) = &self.error {
            ui.colored_label(
                heat::rgb_to_color(theme.palette.critical),
                format!("{}: {error}", strings.text(Key::ViewProcesses)),
            );
        }
        if let Some(error) = &self.details_error {
            ui.colored_label(
                heat::rgb_to_color(theme.palette.warning),
                format!("{}: {error}", strings.text(Key::ProcessInfo)),
            );
        }
        if let Some(notice) = &self.notice {
            ui.colored_label(ok, notice);
        }
    }

    /// Дерево процессов.
    fn show_tree(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let theme = self.theme.clone();
        let Some(snapshot) = self.take_snapshot() else {
            ui.label(strings.text(Key::Collecting));
            self.restore_snapshot(None);
            return;
        };

        let process_tree = ProcessTree::build(&snapshot);

        // Развернуть и свернуть всё — как «Expand Process Tree» в Task Explorer.
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new(strings.text(Key::ExpandAll)).size(12.0))
                .clicked()
            {
                self.collapsed.clear();
            }
            if ui
                .button(egui::RichText::new(strings.text(Key::CollapseAll)).size(12.0))
                .clicked()
            {
                self.collapsed = snapshot
                    .processes
                    .iter()
                    .filter(|process| !process_tree.children_of(process.key.pid).is_empty())
                    .map(|process| process.key.pid)
                    .collect();
            }
        });
        ui.add_space(4.0);

        let rows = tree::flatten(&process_tree, &snapshot.processes, &self.collapsed);

        // Колонки фиксированы: значения должны стоять колонкой, а не
        // прижиматься к правому краю каждой строки отдельно.
        const CPU_WIDTH: f32 = 70.0;
        const MEMORY_WIDTH: f32 = 96.0;
        const ROW_HEIGHT: f32 = 22.0;
        const INDENT: f32 = 14.0;
        const MARKER_SIZE: f32 = 12.0;

        let muted = heat::rgb_to_color(theme.palette.muted);
        let accent = heat::rgb_to_color(theme.palette.accent);
        let text = heat::rgb_to_color(theme.palette.text);
        let grid = heat::rgb_to_color(theme.palette.grid);

        let mut toggle: Option<u32> = None;
        let mut pick: Option<u32> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width();
            let cpu_x = width - MEMORY_WIDTH - CPU_WIDTH;
            let memory_x = width - MEMORY_WIDTH;

            // Шапка колонок.
            let (header, _) =
                ui.allocate_exact_size(egui::Vec2::new(width, 24.0), egui::Sense::hover());
            let painter = ui.painter().with_clip_rect(header);
            painter.text(
                header.left_center() + egui::Vec2::new(22.0, 0.0),
                egui::Align2::LEFT_CENTER,
                strings.text(Key::ColumnName),
                egui::FontId::proportional(11.0),
                muted,
            );
            painter.text(
                egui::pos2(cpu_x + CPU_WIDTH - 4.0, header.center().y),
                egui::Align2::RIGHT_CENTER,
                strings.text(Key::ColumnCpu),
                egui::FontId::proportional(11.0),
                muted,
            );
            painter.text(
                egui::pos2(memory_x + MEMORY_WIDTH - 4.0, header.center().y),
                egui::Align2::RIGHT_CENTER,
                strings.text(Key::ColumnMemory),
                egui::FontId::proportional(11.0),
                muted,
            );
            painter.line_segment(
                [header.left_bottom(), header.right_bottom()],
                egui::Stroke::new(1.0, grid),
            );

            if rows.is_empty() {
                ui.label(strings.text(Key::NoData));
                return;
            }

            let height = ROW_HEIGHT * rows.len() as f32;
            let (rect, _) =
                ui.allocate_exact_size(egui::Vec2::new(width, height), egui::Sense::hover());
            let painter = ui.painter().with_clip_rect(rect);
            let tree_id = ui.scope_id().with("tree_rows");

            for (index, row) in rows.iter().enumerate() {
                let top = rect.min.y + ROW_HEIGHT * index as f32;
                let row_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, top),
                    egui::Vec2::new(width, ROW_HEIGHT),
                );

                let is_selected = self.selected == Some(row.pid);
                if is_selected {
                    painter.rect_filled(
                        row_rect,
                        egui::CornerRadius::from(4.0),
                        accent.gamma_multiply(0.20),
                    );
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(row_rect.min.x, row_rect.min.y + 2.0),
                            egui::Vec2::new(2.0, row_rect.height() - 4.0),
                        ),
                        egui::CornerRadius::from(1.0),
                        accent,
                    );
                }

                let indent = row.depth as f32 * INDENT;

                // Маркер раскрытия ветки.
                let marker = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + indent + 4.0, top),
                    egui::Vec2::new(MARKER_SIZE, ROW_HEIGHT),
                );
                if row.expandable {
                    let response = ui.interact(
                        marker,
                        tree_id.with(("marker", row.pid)),
                        egui::Sense::click(),
                    );
                    widgets::chevron(&painter, marker, !row.collapsed, muted);
                    if response.clicked() {
                        toggle = Some(row.pid);
                    }
                } else {
                    painter.circle_filled(marker.center(), 1.6, muted);
                }

                // Имя процесса.
                let name_width = (width - indent - 22.0 - CPU_WIDTH - MEMORY_WIDTH).max(40.0);
                let name_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x + indent + 22.0, top),
                    egui::Vec2::new(name_width, ROW_HEIGHT),
                );
                let response = ui.interact(
                    name_rect,
                    tree_id.with(("name", row.pid)),
                    egui::Sense::click(),
                );
                let label = format!("{}  ({})", row.name, row.pid);
                painter.text(
                    name_rect.left_center() + egui::Vec2::new(2.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    if is_selected { accent } else { text },
                );
                if response.clicked() {
                    pick = Some(row.pid);
                }

                // Доля CPU и память.
                let cpu = match row.cpu {
                    Some(value) => format!("{value:.1}"),
                    None => "—".to_string(),
                };
                painter.text(
                    egui::pos2(cpu_x + CPU_WIDTH - 4.0, row_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    cpu,
                    egui::FontId::monospace(11.5),
                    if row.cpu.is_some() { text } else { muted },
                );

                let memory = match row.memory {
                    Some(value) => format!("{:.1}", value as f64 / 1_048_576.0),
                    None => "—".to_string(),
                };
                painter.text(
                    egui::pos2(memory_x + MEMORY_WIDTH - 4.0, row_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    memory,
                    egui::FontId::monospace(11.5),
                    if row.memory.is_some() { text } else { muted },
                );
            }
        });

        if let Some(pid) = toggle {
            if self.collapsed.contains(&pid) {
                self.collapsed.remove(&pid);
            } else {
                self.collapsed.insert(pid);
            }
        }
        if let Some(pid) = pick {
            self.selected = Some(pid);
        }

        self.restore_snapshot(Some(snapshot));
    }

    /// Раздел «Производительность»: крупные графики и топ процессов.
    fn show_performance(&mut self, ui: &mut egui::Ui) {
        let strings = self.strings;
        let theme = self.theme.clone();
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
        ui.add_space(6.0);

        let accent = heat::rgb_to_color(theme.palette.accent);
        let ok = heat::rgb_to_color(theme.palette.ok);
        let warning = heat::rgb_to_color(theme.palette.warning);

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
                190.0,
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
                190.0,
            );
        }

        // Топ процессов по CPU.
        ui.separator();
        ui.label(
            egui::RichText::new(strings.text(Key::ChartTopCpu))
                .size(13.0)
                .strong(),
        );

        let Some(snapshot) = self.take_snapshot() else {
            ui.label(strings.text(Key::Collecting));
            self.restore_snapshot(None);
            return;
        };

        // Топ строится по текущему снимку: в списке только живые процессы и
        // только те, у кого CPU действительно измерен. Завершившийся процесс
        // не должен оставаться здесь со своим старым значением.
        let mut rows: Vec<&ProcessInfo> = snapshot
            .processes
            .iter()
            .filter(|process| process.cpu_percent.value.is_some())
            .collect();
        table::sort_rows(&mut rows, &self.perf_table);
        rows.truncate(TOP_ROWS);

        // Мини-графики собираются заранее: история не должна быть заимствована
        // во время отрисовки строк.
        let sparklines: HashMap<u32, Vec<Option<f64>>> = rows
            .iter()
            .filter_map(|process| {
                let values = self
                    .perf
                    .recent_values(process.key.pid, table::SPARK_SAMPLES);
                if values.is_empty() {
                    None
                } else {
                    Some((process.key.pid, values))
                }
            })
            .collect();

        let columns = self.perf_table.visible.clone();
        let layout = table::Layout::new(&columns);
        let memory_total = snapshot.system.memory_total.value;

        // Топ рисуется той же таблицей, что и список процессов.
        let table_rows: Vec<table::TableRow> = rows
            .iter()
            .map(|process| table::TableRow::Process(process))
            .collect();

        egui::ScrollArea::horizontal().show(ui, |ui| {
            table::header(ui, &theme, strings, &mut self.perf_table, &layout);
            if let Some(pid) = table::rows(
                ui,
                &theme,
                strings,
                &columns,
                &layout,
                &table_rows,
                memory_total,
                &sparklines,
                self.selected,
                false,
            ) {
                self.selected = Some(pid);
            }
        });

        self.restore_snapshot(Some(snapshot));

        // График выбранного процесса — под топом: выбор строки не должен
        // сдвигать сам список.
        let selected_series = self
            .selected
            .and_then(|pid| self.perf.process_series(pid).cloned());
        if let Some(series) = selected_series {
            let pid = self.selected.unwrap_or_default();
            let name = self
                .perf
                .name(pid)
                .map(str::to_string)
                .or_else(|| {
                    let snapshot = if self.frozen {
                        self.frozen_snapshot.as_ref()
                    } else {
                        self.snapshot.as_ref()
                    };
                    snapshot
                        .and_then(|snapshot| snapshot.process(pid))
                        .map(|process| process.name.clone())
                })
                .unwrap_or_else(|| format!("pid {pid}"));
            ui.separator();
            ui.label(egui::RichText::new(format!("{name} ({pid})")).size(13.0));
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
                160.0,
            );
        }
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
                heat::rgb_to_color(self.theme.palette.warning),
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
        let theme = &self.theme;
        let probe = self.sampler.probe();

        ui.label(
            egui::RichText::new(format!(
                "{}: {}",
                strings.text(Key::ViewCapabilities),
                probe.platform()
            ))
            .size(13.0),
        );
        ui.add_space(6.0);

        let ok = heat::rgb_to_color(theme.palette.ok);
        let warning = heat::rgb_to_color(theme.palette.warning);
        let muted = heat::rgb_to_color(theme.palette.muted);
        let surface = heat::rgb_to_color(theme.palette.surface);
        let edge = heat::rgb_to_color(theme.palette.edge);

        // Сводка: сколько возможностей работает полностью, частично и никак.
        let mut full = 0usize;
        let mut partial = 0usize;
        let mut missing = 0usize;
        for capability in Capability::all() {
            match probe.support(*capability) {
                Support::Full => full += 1,
                Support::Partial => partial += 1,
                Support::Unsupported(_) => missing += 1,
            }
        }
        ui.horizontal(|ui| {
            widgets::chip(
                ui,
                theme,
                &format!("{}: {full}", strings.text(Key::SupportFull)),
                ok,
            );
            widgets::chip(
                ui,
                theme,
                &format!("{}: {partial}", strings.text(Key::SupportPartial)),
                warning,
            );
            widgets::chip(
                ui,
                theme,
                &format!("{}: {missing}", strings.text(Key::SupportNone)),
                muted,
            );
        });
        ui.add_space(10.0);

        // Карточки возможностей.
        const CAPABILITY_COLUMNS: usize = 3;
        egui::Grid::new("capabilities")
            .num_columns(CAPABILITY_COLUMNS)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for chunk in Capability::all().chunks(CAPABILITY_COLUMNS) {
                    for capability in chunk {
                        let support = probe.support(*capability);
                        let (text, color) = match support {
                            Support::Full => (strings.text(Key::SupportFull).to_string(), ok),
                            Support::Partial => {
                                (strings.text(Key::SupportPartial).to_string(), warning)
                            }
                            Support::Unsupported(reason) => (
                                format!("{}: {}", strings.text(Key::SupportNone), reason.as_str()),
                                muted,
                            ),
                        };
                        egui::Frame::new()
                            .fill(surface)
                            .stroke(egui::Stroke::new(1.0, edge))
                            .corner_radius(widgets::RADIUS)
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(200.0);
                                ui.label(egui::RichText::new(capability.as_str()).size(11.5));
                                ui.label(egui::RichText::new(text).size(10.5).color(color));
                            });
                    }
                    ui.end_row();
                }
            });

        let limitations = probe.limitations();
        if !limitations.is_empty() {
            ui.add_space(10.0);
            widgets::section_title(ui, theme, strings.text(Key::Limitations));
            for (capability, reason) in limitations {
                ui.label(
                    egui::RichText::new(format!("{}: {reason}", capability.as_str()))
                        .size(11.5)
                        .color(muted),
                );
            }
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
    height: f32,
) {
    let from = timeline.start(now);
    let to = timeline.end(now);

    // Отсчёты копируются: так график не держит заимствование истории.
    let owned: Vec<Sample> = series.window(from, to).into_iter().copied().collect();
    let samples: Vec<&Sample> = owned.iter().collect();

    let text = heat::rgb_to_color(theme.palette.text);
    let muted = heat::rgb_to_color(theme.palette.muted);
    ui.label(
        egui::RichText::new(request.title)
            .size(12.5)
            .strong()
            .color(text),
    );
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            series
                .last_value()
                .map(|value| format!("{value:.1}"))
                .unwrap_or_default(),
            request.unit
        ))
        .size(11.0)
        .monospace()
        .color(muted),
    );

    let spec = ChartSpec::new(request.title, request.unit, request.color);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
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

/// Подпись профиля столбцов.
fn profile_label(strings: Strings, profile: columns::Profile) -> &'static str {
    match profile {
        columns::Profile::Basic => strings.text(Key::ProfileBasic),
        columns::Profile::Cpu => strings.text(Key::ProfileCpu),
        columns::Profile::Memory => strings.text(Key::ProfileMemory),
        columns::Profile::Io => strings.text(Key::ProfileIo),
    }
}

/// Подпись способа группировки.
fn group_label(strings: Strings, by: group::GroupBy) -> &'static str {
    match by {
        group::GroupBy::None => strings.text(Key::GroupNone),
        group::GroupBy::Application => strings.text(Key::GroupApplication),
        group::GroupBy::Parent => strings.text(Key::GroupParent),
    }
}

/// Подпись класса приоритета.
fn priority_label(strings: Strings, priority: Priority) -> &'static str {
    match priority {
        Priority::Low => strings.text(Key::PriorityLow),
        Priority::BelowNormal => strings.text(Key::PriorityBelowNormal),
        Priority::Normal => strings.text(Key::PriorityNormal),
        Priority::AboveNormal => strings.text(Key::PriorityAboveNormal),
        Priority::High => strings.text(Key::PriorityHigh),
        Priority::RealTime => strings.text(Key::PriorityRealTime),
    }
}

/// Сумма счётчиков и число процессов без значения.
fn sum_counters<'a>(counters: impl Iterator<Item = &'a Counter<u32>>) -> (u64, usize) {
    let mut sum = 0u64;
    let mut unknown = 0usize;
    for counter in counters {
        match counter.value {
            Some(value) => sum += value as u64,
            None => unknown += 1,
        }
    }
    (sum, unknown)
}

/// Время снимка в виде «чч:мм:сс».
fn format_clock(unix_ms: u64) -> String {
    let seconds = unix_ms / 1000;
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let seconds = seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Запускает окно монитора.
pub fn run(
    theme_path: Option<PathBuf>,
    lang: Lang,
    view: View,
    group_by: group::GroupBy,
) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("MCPad"),
        ..Default::default()
    };

    eframe::run_native(
        "MCPad",
        options,
        Box::new(move |cc| {
            // Путь копируется: замыкание создателя может быть вызвано как
            // `Fn`, а двигать из него значение нельзя.
            let app = App::new(theme_path.clone(), lang, view, group_by)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?;
            // Тема применяется сразу, до первого кадра.
            theme_ui::apply(&cc.egui_ctx, &app.theme);
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
