#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use eframe::egui;

const NORMAL_WINDOW_WIDTH: f32 = 440.0;
const NORMAL_WINDOW_HEIGHT: f32 = 520.0;
const COLLAPSED_THICKNESS: f32 = 22.0;
const COLLAPSED_LENGTH: f32 = 96.0;
const EXPANDED_WIDTH: f32 = 350.0;
const EXPANDED_HEIGHT: f32 = 430.0;
const MIN_EXPANDED_WIDTH: f32 = 260.0;
const MIN_EXPANDED_HEIGHT: f32 = 260.0;
const MAX_EXPANDED_WIDTH: f32 = 900.0;
const MAX_EXPANDED_HEIGHT: f32 = 900.0;
const TITLE_BAR_HEIGHT: f32 = 32.0;
const INITIAL_Y: f32 = 260.0;
const COLLAPSE_DELAY_MILLISECONDS: u64 = 1000;
const RESIZE_HANDLE_THICKNESS: f32 = 12.0;
const TOAST_DURATION_MILLISECONDS: u64 = 1800;
const SCREEN_EDGE_DROP_DISTANCE: f32 = 96.0;
const CONFIGURATION_DIRECTORY_NAME: &str = "env";
const CONFIGURATION_FILE_NAME: &str = "quick_dock.ini";
const LOG_FILE_NAME: &str = "quick_dock.log";
const SCHEMA_VERSION: u32 = 1;
const AUTOSTART_VALUE_NAME: &str = "QuickDock";

fn main() -> eframe::Result {
    let Some(_single_instance_guard) = SingleInstanceGuard::acquire() else {
        return Ok(());
    };

    log_event("Quick Dock 시작");

    let initial_size = egui::vec2(NORMAL_WINDOW_WIDTH, NORMAL_WINDOW_HEIGHT);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quick Dock")
            .with_inner_size(initial_size)
            .with_min_inner_size(egui::vec2(360.0, 360.0))
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(true)
            .with_taskbar(true),
        ..Default::default()
    };

    eframe::run_native(
        "Quick Dock",
        native_options,
        Box::new(|creation_context| Ok(Box::new(QuickDockApplication::new(creation_context)))),
    )
}

#[cfg(target_os = "windows")]
struct SingleInstanceGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl SingleInstanceGuard {
    fn acquire() -> Option<Self> {
        use std::ffi::OsStr;
        use std::ptr::null;
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let mutex_name = wide_null(OsStr::new("Local\\QuickDockSingleInstance"));
        let handle = unsafe { CreateMutexW(null(), 1, mutex_name.as_ptr()) };

        if handle.is_null() {
            return Some(Self { handle });
        }

        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            focus_existing_instance_window();
            unsafe {
                CloseHandle(handle);
            }
            return None;
        }

        Some(Self { handle })
    }
}

#[cfg(target_os = "windows")]
fn focus_existing_instance_window() {
    use std::ffi::OsStr;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let title = wide_null(OsStr::new("Quick Dock"));
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if !window.is_null() {
            ShowWindow(window, SW_SHOW);
            ShowWindow(window, SW_RESTORE);
            SetForegroundWindow(window);
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn wide_null(text: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
struct SingleInstanceGuard;

#[cfg(not(target_os = "windows"))]
impl SingleInstanceGuard {
    fn acquire() -> Option<Self> {
        Some(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockEdge {
    fn korean_name(self) -> &'static str {
        match self {
            DockEdge::Left => "왼쪽",
            DockEdge::Right => "오른쪽",
            DockEdge::Top => "위쪽",
            DockEdge::Bottom => "아래쪽",
        }
    }

    fn ini_value(self) -> &'static str {
        match self {
            DockEdge::Left => "left",
            DockEdge::Right => "right",
            DockEdge::Top => "top",
            DockEdge::Bottom => "bottom",
        }
    }

    fn from_ini_value(value: &str) -> Option<DockEdge> {
        match value.trim().to_ascii_lowercase().as_str() {
            "left" => Some(DockEdge::Left),
            "right" => Some(DockEdge::Right),
            "top" => Some(DockEdge::Top),
            "bottom" => Some(DockEdge::Bottom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum ActionItem {
    CopyText {
        name: String,
        text: String,
    },
    RunApplication {
        name: String,
        command: String,
        arguments: Vec<String>,
    },
    OpenPath {
        name: String,
        path: String,
    },
}

impl ActionItem {
    fn name(&self) -> &str {
        match self {
            ActionItem::CopyText { name, .. } => name,
            ActionItem::RunApplication { name, .. } => name,
            ActionItem::OpenPath { name, .. } => name,
        }
    }

    fn kind(&self) -> ActionKind {
        match self {
            ActionItem::CopyText { .. } => ActionKind::CopyText,
            ActionItem::RunApplication { .. } => ActionKind::RunApplication,
            ActionItem::OpenPath { .. } => ActionKind::OpenPath,
        }
    }

    fn search_payload(&self) -> String {
        match self {
            ActionItem::CopyText { text, .. } => text.clone(),
            ActionItem::RunApplication {
                command, arguments, ..
            } => format!("{command} {}", arguments.join(" ")),
            ActionItem::OpenPath { path, .. } => path.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    CopyText,
    RunApplication,
    OpenPath,
}

impl ActionKind {
    fn label(self) -> &'static str {
        match self {
            ActionKind::CopyText => "복사",
            ActionKind::RunApplication => "실행",
            ActionKind::OpenPath => "열기",
        }
    }

    fn ini_value(self) -> &'static str {
        match self {
            ActionKind::CopyText => "copy_text",
            ActionKind::RunApplication => "run_app",
            ActionKind::OpenPath => "open_path",
        }
    }
}

#[derive(Debug, Clone)]
struct EditableActionItem {
    kind: ActionKind,
    name: String,
    text: String,
    command: String,
    arguments: String,
    path: String,
}

impl EditableActionItem {
    fn blank(kind: ActionKind) -> Self {
        Self {
            kind,
            name: "새 항목".to_owned(),
            text: String::new(),
            command: String::new(),
            arguments: String::new(),
            path: String::new(),
        }
    }

    fn from_action_item(item: &ActionItem) -> Self {
        match item {
            ActionItem::CopyText { name, text } => Self {
                kind: ActionKind::CopyText,
                name: name.clone(),
                text: text.clone(),
                command: String::new(),
                arguments: String::new(),
                path: String::new(),
            },
            ActionItem::RunApplication {
                name,
                command,
                arguments,
            } => Self {
                kind: ActionKind::RunApplication,
                name: name.clone(),
                text: String::new(),
                command: command.clone(),
                arguments: arguments.join("|"),
                path: String::new(),
            },
            ActionItem::OpenPath { name, path } => Self {
                kind: ActionKind::OpenPath,
                name: name.clone(),
                text: String::new(),
                command: String::new(),
                arguments: String::new(),
                path: path.clone(),
            },
        }
    }

    fn to_action_item(&self) -> ActionItem {
        match self.kind {
            ActionKind::CopyText => ActionItem::CopyText {
                name: self.name.clone(),
                text: self.text.clone(),
            },
            ActionKind::RunApplication => ActionItem::RunApplication {
                name: self.name.clone(),
                command: self.command.clone(),
                arguments: parse_argument_list(&self.arguments),
            },
            ActionKind::OpenPath => ActionItem::OpenPath {
                name: self.name.clone(),
                path: self.path.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationSeverity {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
struct ValidationMessage {
    severity: ValidationSeverity,
    text: String,
}

impl ValidationMessage {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Ok,
            text: text.into(),
        }
    }

    fn warning(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            text: text.into(),
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone)]
enum EditorActionRequest {
    Test(ActionItem),
    Status { message: String, is_error: bool },
}

#[derive(Debug, Clone, Copy)]
enum ButtonActionRequest {
    Execute(usize),
    RunAsAdmin(usize),
    OpenItemLocation(usize),
    CopyItemPath(usize),
    EditItem(usize),
    DuplicateItem(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
}

#[derive(Debug, Clone)]
struct QuickDockTab {
    name: String,
    items: Vec<ActionItem>,
    editable_items: Vec<EditableActionItem>,
}

impl QuickDockTab {
    fn new(name: String, items: Vec<ActionItem>) -> Self {
        let editable_items = items
            .iter()
            .map(EditableActionItem::from_action_item)
            .collect();

        Self {
            name,
            items,
            editable_items,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DragDropTarget {
    Window {
        monitor_rect: egui::Rect,
    },
    Dock {
        edge: DockEdge,
        monitor_rect: egui::Rect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeEdge {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn cursor_icon(self) -> egui::CursorIcon {
        match self {
            ResizeEdge::TopLeft => egui::CursorIcon::ResizeNorthWest,
            ResizeEdge::TopRight => egui::CursorIcon::ResizeNorthEast,
            ResizeEdge::BottomLeft => egui::CursorIcon::ResizeSouthWest,
            ResizeEdge::BottomRight => egui::CursorIcon::ResizeSouthEast,
        }
    }

    fn affects_left(self) -> bool {
        matches!(self, ResizeEdge::TopLeft | ResizeEdge::BottomLeft)
    }

    fn affects_right(self) -> bool {
        matches!(self, ResizeEdge::TopRight | ResizeEdge::BottomRight)
    }

    fn affects_top(self) -> bool {
        matches!(self, ResizeEdge::TopLeft | ResizeEdge::TopRight)
    }

    fn affects_bottom(self) -> bool {
        matches!(self, ResizeEdge::BottomLeft | ResizeEdge::BottomRight)
    }

    fn id_salt(self) -> &'static str {
        match self {
            ResizeEdge::TopLeft => "top_left",
            ResizeEdge::TopRight => "top_right",
            ResizeEdge::BottomLeft => "bottom_left",
            ResizeEdge::BottomRight => "bottom_right",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResizeDrag {
    edge: ResizeEdge,
    start_window_rect: egui::Rect,
}

#[derive(Debug, Clone, Copy)]
struct MonitorDockState {
    edge: DockEdge,
    anchor: egui::Pos2,
    expanded_size: egui::Vec2,
}

#[derive(Debug, Clone)]
struct LayoutSettings {
    expanded_size: egui::Vec2,
    window_size: egui::Vec2,
    dock_edge: DockEdge,
    monitors: BTreeMap<String, MonitorDockState>,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            expanded_size: default_expanded_size(),
            window_size: egui::vec2(NORMAL_WINDOW_WIDTH, NORMAL_WINDOW_HEIGHT),
            dock_edge: DockEdge::Left,
            monitors: BTreeMap::new(),
        }
    }
}

struct QuickDockApplication {
    configuration_path: PathBuf,
    tabs: Vec<QuickDockTab>,
    active_tab_index: usize,
    next_tab_number: usize,
    new_item_kind: ActionKind,
    dock_edge: DockEdge,
    highlighted_dock_edge: Option<DockEdge>,
    drag_drop_target: Option<DragDropTarget>,
    drag_pointer_offset: egui::Vec2,
    expanded_resize_drag: Option<ResizeDrag>,
    window_resize_drag: Option<ResizeDrag>,
    expanded_size: egui::Vec2,
    window_size: egui::Vec2,
    monitor_dock_states: BTreeMap<String, MonitorDockState>,
    renaming_tab_index: Option<usize>,
    renaming_focus_pending: bool,
    is_settings_editor_open: bool,
    is_docked: bool,
    is_expanded: bool,
    is_dragging: bool,
    last_hovered_at: Instant,
    last_status_message: String,
    toast_message: Option<String>,
    toast_started_at: Instant,
    toast_is_error: bool,
    last_known_position: egui::Pos2,
    last_known_inner_position: egui::Pos2,
    last_known_monitor_rect: egui::Rect,
    available_monitor_rects: Vec<egui::Rect>,
    dock_anchor_position: egui::Pos2,
    pending_input_copy: Option<PendingInputCopy>,
    last_external_foreground_window: Option<isize>,
    editor_focus_index: Option<usize>,
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    palette_focus_pending: bool,
    recent_items: Vec<ActionItem>,
    autostart_enabled: bool,
    #[cfg(target_os = "windows")]
    tray_state: Option<TrayState>,
    #[cfg(target_os = "windows")]
    tray_attempted: bool,
}

#[cfg(target_os = "windows")]
struct TrayState {
    _icon: tray_icon::TrayIcon,
    autostart_item: tray_icon::menu::CheckMenuItem,
}

struct PaletteEntry {
    item: ActionItem,
    tab_index: Option<usize>,
    tab_label: String,
    is_recent: bool,
}

#[derive(Debug, Clone)]
struct PendingInputCopy {
    name: String,
    template: String,
    fields: Vec<TemplateInputField>,
    focus_pending: bool,
}

#[derive(Debug, Clone)]
struct TemplateInputField {
    label: String,
    value: String,
}

impl QuickDockApplication {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        configure_system_fonts(&creation_context.egui_ctx);

        let configuration_path = get_configuration_path();
        let (tabs, layout, status_message) = load_tabs_from_configuration(&configuration_path);
        let next_tab_number = tabs.len() + 1;
        let LayoutSettings {
            expanded_size,
            window_size,
            dock_edge,
            monitors,
        } = layout;

        Self {
            configuration_path,
            tabs,
            active_tab_index: 0,
            next_tab_number,
            new_item_kind: ActionKind::CopyText,
            dock_edge,
            highlighted_dock_edge: None,
            drag_drop_target: None,
            drag_pointer_offset: egui::vec2(24.0, 16.0),
            expanded_resize_drag: None,
            window_resize_drag: None,
            expanded_size,
            window_size,
            monitor_dock_states: monitors,
            renaming_tab_index: None,
            renaming_focus_pending: false,
            is_settings_editor_open: false,
            is_docked: false,
            is_expanded: true,
            is_dragging: false,
            last_hovered_at: Instant::now(),
            last_status_message: status_message,
            toast_message: None,
            toast_started_at: Instant::now(),
            toast_is_error: false,
            last_known_position: egui::pos2(0.0, INITIAL_Y),
            last_known_inner_position: egui::pos2(0.0, INITIAL_Y),
            last_known_monitor_rect: egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1920.0, 1080.0),
            ),
            available_monitor_rects: vec![egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1920.0, 1080.0),
            )],
            dock_anchor_position: egui::pos2(0.0, INITIAL_Y + COLLAPSED_LENGTH * 0.5),
            pending_input_copy: None,
            last_external_foreground_window: None,
            editor_focus_index: None,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            palette_focus_pending: false,
            recent_items: Vec::new(),
            autostart_enabled: is_autostart_enabled(),
            #[cfg(target_os = "windows")]
            tray_state: None,
            #[cfg(target_os = "windows")]
            tray_attempted: false,
        }
    }

    fn reload_configuration(&mut self) {
        let (tabs, layout, status_message) =
            load_tabs_from_configuration(&self.configuration_path);
        self.tabs = tabs;
        self.expanded_size = layout.expanded_size;
        self.window_size = layout.window_size;
        self.dock_edge = layout.dock_edge;
        self.monitor_dock_states = layout.monitors;
        self.active_tab_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        self.next_tab_number = self.tabs.len() + 1;
        self.renaming_tab_index = None;
        self.renaming_focus_pending = false;
        self.last_status_message = status_message;
    }

    fn update_window_state(&mut self, context: &egui::Context, frame: &eframe::Frame) {
        let viewport_info = context.input(|input| input.viewport().clone());

        self.update_monitor_rect(context, frame, &viewport_info);

        if let Some(external_window) = current_external_foreground_window() {
            self.last_external_foreground_window = Some(external_window);
        }

        if let Some(outer_rect) = viewport_info.outer_rect {
            self.last_known_position = outer_rect.min;
        }

        if let Some(inner_rect) = viewport_info.inner_rect {
            self.last_known_inner_position = inner_rect.min;
        }

        if self.is_dragging {
            self.update_drag_preview(context);
            context.request_repaint_after(Duration::from_millis(16));
            return;
        }

        if self.expanded_resize_drag.is_some() {
            self.update_expanded_resize(context);
            context.request_repaint_after(Duration::from_millis(16));
            return;
        }

        if self.window_resize_drag.is_some() {
            self.update_window_resize(context);
            context.request_repaint_after(Duration::from_millis(16));
            return;
        }

        if !self.is_docked {
            context.request_repaint_after(Duration::from_millis(80));
            return;
        }

        if self.is_settings_editor_open || self.pending_input_copy.is_some() || self.palette_open {
            self.last_hovered_at = Instant::now();
            if !self.is_expanded {
                self.is_expanded = true;
                self.apply_dock_geometry(context);
            }
            context.request_repaint_after(Duration::from_millis(80));
            return;
        }

        let is_pointer_inside_window = context.input(|input| input.pointer.hover_pos().is_some());

        if is_pointer_inside_window {
            self.last_hovered_at = Instant::now();
        }

        let should_expand = is_pointer_inside_window;
        let should_collapse = !is_pointer_inside_window
            && self.last_hovered_at.elapsed() >= Duration::from_millis(COLLAPSE_DELAY_MILLISECONDS);

        if should_expand && !self.is_expanded {
            self.is_expanded = true;
            self.apply_dock_geometry(context);
        } else if should_collapse && self.is_expanded {
            self.is_expanded = false;
            self.apply_dock_geometry(context);
        }

        context.request_repaint_after(Duration::from_millis(80));
    }

    fn update_monitor_rect(
        &mut self,
        context: &egui::Context,
        frame: &eframe::Frame,
        viewport_info: &egui::ViewportInfo,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(window) = frame.winit_window() {
            let pixels_per_point = context.pixels_per_point().max(1.0);
            let monitor_rects: Vec<egui::Rect> = window
                .available_monitors()
                .map(|monitor| {
                    let position = monitor.position();
                    let size = monitor.size();
                    egui::Rect::from_min_size(
                        egui::pos2(
                            position.x as f32 / pixels_per_point,
                            position.y as f32 / pixels_per_point,
                        ),
                        egui::vec2(
                            size.width as f32 / pixels_per_point,
                            size.height as f32 / pixels_per_point,
                        ),
                    )
                })
                .collect();

            if !monitor_rects.is_empty() {
                self.available_monitor_rects = monitor_rects;
            }

            if self.is_dragging {
                if let Some(cursor_position) = self.current_cursor_position(context) {
                    self.last_known_monitor_rect = self.monitor_rect_for_position(cursor_position);
                    return;
                }
            }

            if let Some(monitor) = window.current_monitor() {
                let position = monitor.position();
                let size = monitor.size();
                let monitor_position = egui::pos2(
                    position.x as f32 / pixels_per_point,
                    position.y as f32 / pixels_per_point,
                );
                let monitor_size = egui::vec2(
                    size.width as f32 / pixels_per_point,
                    size.height as f32 / pixels_per_point,
                );

                self.last_known_monitor_rect =
                    egui::Rect::from_min_size(monitor_position, monitor_size);
                return;
            }
        }

        if let Some(monitor_size) = viewport_info.monitor_size {
            self.last_known_monitor_rect =
                egui::Rect::from_min_size(self.last_known_monitor_rect.min, monitor_size);
        }
    }

    fn begin_title_bar_drag(&mut self, context: &egui::Context) {
        let cursor_position = self
            .current_cursor_position(context)
            .unwrap_or(self.last_known_position + self.drag_pointer_offset);
        self.drag_pointer_offset = cursor_position - self.last_known_position;
        self.drag_pointer_offset.x = self
            .drag_pointer_offset
            .x
            .clamp(8.0, NORMAL_WINDOW_WIDTH - 8.0);
        self.drag_pointer_offset.y = self
            .drag_pointer_offset
            .y
            .clamp(8.0, NORMAL_WINDOW_HEIGHT - 8.0);

        self.highlighted_dock_edge = None;
        self.drag_drop_target = None;
        self.is_docked = false;
        self.is_expanded = true;
        self.is_dragging = true;
        self.last_status_message = "상단바를 놓으면 위치가 확정됩니다.".to_owned();

        context.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        context.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
        context.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        self.update_drag_preview(context);
    }

    fn update_drag_preview(&mut self, context: &egui::Context) {
        let pointer_released = context.input(|input| input.pointer.any_released());
        let primary_down = is_primary_mouse_button_down().unwrap_or_else(|| {
            context.input(|input| input.pointer.button_down(egui::PointerButton::Primary))
        });

        if pointer_released || !primary_down {
            self.finish_title_bar_drag(context);
            return;
        }

        let cursor_position = self
            .current_cursor_position(context)
            .unwrap_or(self.last_known_position + self.drag_pointer_offset);
        let monitor_rect = self.monitor_rect_for_position(cursor_position);
        self.last_known_monitor_rect = monitor_rect;

        if let Some(edge) = edge_from_monitor_position(cursor_position, monitor_rect) {
            self.highlighted_dock_edge = Some(edge);
            self.drag_drop_target = Some(DragDropTarget::Dock { edge, monitor_rect });
            self.dock_edge = edge;
            self.dock_anchor_position = clamp_pos_to_rect(cursor_position, monitor_rect);
            self.is_expanded = false;

            let target_size = get_window_size(edge, false);
            let target_position =
                get_docked_position(edge, cursor_position, monitor_rect, target_size);
            self.send_drag_preview_geometry(context, target_size, target_position);
            self.last_status_message = format!("{} 도킹 미리보기", edge.korean_name());
        } else {
            self.highlighted_dock_edge = None;
            self.drag_drop_target = Some(DragDropTarget::Window { monitor_rect });
            self.is_expanded = true;

            let target_size = self.window_size;
            let target_position = normal_window_position_for_cursor(
                cursor_position,
                self.drag_pointer_offset,
                monitor_rect,
                target_size,
            );
            self.send_drag_preview_geometry(context, target_size, target_position);
            self.last_status_message = "창 모드 미리보기".to_owned();
        }
    }

    fn finish_title_bar_drag(&mut self, context: &egui::Context) {
        let cursor_position = self
            .current_cursor_position(context)
            .unwrap_or(self.dock_anchor_position);
        let target = self.drag_drop_target.take().unwrap_or_else(|| {
            let monitor_rect = self.monitor_rect_for_position(cursor_position);
            if let Some(edge) = edge_from_monitor_position(cursor_position, monitor_rect) {
                DragDropTarget::Dock { edge, monitor_rect }
            } else {
                DragDropTarget::Window { monitor_rect }
            }
        });

        match target {
            DragDropTarget::Dock { edge, monitor_rect } => {
                self.last_known_monitor_rect = monitor_rect;
                self.dock_to_edge_at(edge, cursor_position, context);
            }
            DragDropTarget::Window { monitor_rect } => {
                self.finish_window_mode_at(cursor_position, monitor_rect, context);
            }
        }
    }

    fn finish_window_mode_at(
        &mut self,
        cursor_position: egui::Pos2,
        monitor_rect: egui::Rect,
        context: &egui::Context,
    ) {
        let target_size = self.window_size;
        let target_position = normal_window_position_for_cursor(
            cursor_position,
            self.drag_pointer_offset,
            monitor_rect,
            target_size,
        );

        self.highlighted_dock_edge = None;
        self.drag_drop_target = None;
        self.is_docked = false;
        self.is_expanded = true;
        self.is_dragging = false;
        self.last_known_monitor_rect = monitor_rect;
        self.last_known_position = target_position;
        self.last_status_message = "창 모드로 배치했습니다.".to_owned();

        context.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        context.send_viewport_cmd(egui::ViewportCommand::Transparent(false));
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
        context.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::Normal,
        ));
        context.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
            360.0, 360.0,
        )));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(target_position));
    }

    fn send_drag_preview_geometry(
        &self,
        context: &egui::Context,
        target_size: egui::Vec2,
        target_position: egui::Pos2,
    ) {
        context.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        context.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
        context.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        context.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(1.0, 1.0)));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(target_position));
    }

    fn current_cursor_position(&self, context: &egui::Context) -> Option<egui::Pos2> {
        get_global_cursor_position(context.pixels_per_point()).or_else(|| {
            context
                .input(|input| input.pointer.interact_pos())
                .map(|position| self.pointer_to_monitor_position(position))
        })
    }

    fn monitor_rect_for_position(&self, position: egui::Pos2) -> egui::Rect {
        self.available_monitor_rects
            .iter()
            .copied()
            .find(|rect| rect.expand(2.0).contains(position))
            .or_else(|| {
                self.available_monitor_rects
                    .iter()
                    .copied()
                    .min_by(|left, right| {
                        distance_to_rect(position, *left)
                            .partial_cmp(&distance_to_rect(position, *right))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .unwrap_or(self.last_known_monitor_rect)
    }

    fn begin_expanded_resize(&mut self, edge: ResizeEdge, context: &egui::Context) {
        let start_position = get_docked_position(
            self.dock_edge,
            self.dock_anchor_position,
            self.last_known_monitor_rect,
            self.expanded_size,
        );
        let start_window_rect = egui::Rect::from_min_size(start_position, self.expanded_size);

        self.expanded_resize_drag = Some(ResizeDrag {
            edge,
            start_window_rect,
        });
        self.last_hovered_at = Instant::now();
        self.last_status_message = "창 크기를 조절하는 중입니다.".to_owned();
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
    }

    fn update_expanded_resize(&mut self, context: &egui::Context) {
        let pointer_released = context.input(|input| input.pointer.any_released());
        let primary_down = is_primary_mouse_button_down().unwrap_or_else(|| {
            context.input(|input| input.pointer.button_down(egui::PointerButton::Primary))
        });

        if pointer_released || !primary_down {
            self.finish_expanded_resize(context);
            return;
        }

        let Some(resize_drag) = self.expanded_resize_drag else {
            return;
        };
        let Some(cursor_position) = self.current_cursor_position(context) else {
            return;
        };

        let mut resized_rect = resize_drag.start_window_rect;
        if resize_drag.edge.affects_left() {
            resized_rect.min.x = cursor_position.x;
        }
        if resize_drag.edge.affects_right() {
            resized_rect.max.x = cursor_position.x;
        }
        if resize_drag.edge.affects_top() {
            resized_rect.min.y = cursor_position.y;
        }
        if resize_drag.edge.affects_bottom() {
            resized_rect.max.y = cursor_position.y;
        }

        resized_rect = clamp_resize_rect(
            resized_rect,
            resize_drag.edge,
            self.dock_edge,
            self.last_known_monitor_rect,
        );
        self.expanded_size =
            clamp_expanded_size_to_monitor(resized_rect.size(), self.last_known_monitor_rect);
        self.dock_anchor_position =
            clamp_pos_to_rect(resized_rect.center(), self.last_known_monitor_rect);
        self.is_docked = true;
        self.is_expanded = true;
        self.last_hovered_at = Instant::now();
        self.apply_dock_geometry(context);
    }

    fn finish_expanded_resize(&mut self, context: &egui::Context) {
        self.expanded_resize_drag = None;
        self.expanded_size =
            clamp_expanded_size_to_monitor(self.expanded_size, self.last_known_monitor_rect);
        self.last_hovered_at = Instant::now();
        self.apply_dock_geometry(context);
        self.save_layout_settings();
    }

    fn begin_window_resize(&mut self, edge: ResizeEdge, context: &egui::Context) {
        let start_window_rect = egui::Rect::from_min_size(self.last_known_position, self.window_size);
        self.window_resize_drag = Some(ResizeDrag {
            edge,
            start_window_rect,
        });
        self.last_status_message = "창 크기를 조절하는 중입니다.".to_owned();
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
    }

    fn update_window_resize(&mut self, context: &egui::Context) {
        let pointer_released = context.input(|input| input.pointer.any_released());
        let primary_down = is_primary_mouse_button_down().unwrap_or_else(|| {
            context.input(|input| input.pointer.button_down(egui::PointerButton::Primary))
        });

        if pointer_released || !primary_down {
            self.finish_window_resize(context);
            return;
        }

        let Some(resize_drag) = self.window_resize_drag else {
            return;
        };
        let Some(cursor_position) = self.current_cursor_position(context) else {
            return;
        };

        let mut resized_rect = resize_drag.start_window_rect;
        if resize_drag.edge.affects_left() {
            resized_rect.min.x = cursor_position.x;
        }
        if resize_drag.edge.affects_right() {
            resized_rect.max.x = cursor_position.x;
        }
        if resize_drag.edge.affects_top() {
            resized_rect.min.y = cursor_position.y;
        }
        if resize_drag.edge.affects_bottom() {
            resized_rect.max.y = cursor_position.y;
        }

        resized_rect =
            clamp_free_window_rect(resized_rect, resize_drag.edge, self.last_known_monitor_rect);
        self.window_size = resized_rect.size();
        self.last_known_position = resized_rect.min;

        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.window_size));
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(self.last_known_position));
    }

    fn finish_window_resize(&mut self, context: &egui::Context) {
        self.window_resize_drag = None;
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.window_size));
        self.last_status_message = "창 크기를 저장했습니다.".to_owned();
        self.save_layout_settings();
    }

    fn current_layout(&self) -> LayoutSettings {
        LayoutSettings {
            expanded_size: self.expanded_size,
            window_size: self.window_size,
            dock_edge: self.dock_edge,
            monitors: self.monitor_dock_states.clone(),
        }
    }

    fn record_current_monitor_state(&mut self) {
        let key = monitor_key(self.last_known_monitor_rect);
        self.monitor_dock_states.insert(
            key,
            MonitorDockState {
                edge: self.dock_edge,
                anchor: self.dock_anchor_position,
                expanded_size: self.expanded_size,
            },
        );
    }

    fn restore_monitor_state_for_current(&mut self) {
        let key = monitor_key(self.last_known_monitor_rect);
        if let Some(state) = self.monitor_dock_states.get(&key).copied() {
            self.expanded_size =
                clamp_expanded_size_to_monitor(state.expanded_size, self.last_known_monitor_rect);
        }
    }

    fn save_layout_settings(&mut self) {
        self.record_current_monitor_state();
        let layout = self.current_layout();
        if let Err(error) =
            save_tabs_to_configuration(&self.configuration_path, &self.tabs, &layout, false)
        {
            self.last_status_message = format!("창 크기 저장 실패: {error}");
            log_event(&format!("레이아웃 저장 실패: {error}"));
        }
    }

    fn docked_window_size(&self, is_expanded: bool) -> egui::Vec2 {
        if is_expanded {
            self.expanded_size
        } else {
            get_window_size(self.dock_edge, false)
        }
    }

    fn apply_dock_geometry(&self, context: &egui::Context) {
        let target_size = self.docked_window_size(self.is_expanded);
        let target_position = get_docked_position(
            self.dock_edge,
            self.dock_anchor_position,
            self.last_known_monitor_rect,
            target_size,
        );

        context.send_viewport_cmd(egui::ViewportCommand::Decorations(false));
        context.send_viewport_cmd(egui::ViewportCommand::Transparent(true));
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
        context.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        context.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(1.0, 1.0)));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(target_position));
    }

    fn dock_to_edge_at(
        &mut self,
        edge: DockEdge,
        anchor_position: egui::Pos2,
        context: &egui::Context,
    ) {
        self.dock_edge = edge;
        self.highlighted_dock_edge = None;
        self.is_docked = true;
        self.is_expanded = false;
        self.is_dragging = false;
        self.dock_anchor_position =
            clamp_pos_to_rect(anchor_position, self.last_known_monitor_rect);
        self.restore_monitor_state_for_current();
        self.last_known_position = get_docked_position(
            edge,
            self.dock_anchor_position,
            self.last_known_monitor_rect,
            get_window_size(edge, false),
        );
        self.last_status_message = format!("{}에 도킹했습니다.", edge.korean_name());
        self.apply_dock_geometry(context);
        self.save_layout_settings();
    }

    fn add_tab(&mut self) {
        let tab_name = format!("탭 {}", self.next_tab_number);
        self.next_tab_number += 1;
        self.tabs.push(QuickDockTab::new(tab_name, Vec::new()));
        self.active_tab_index = self.tabs.len().saturating_sub(1);
        self.renaming_tab_index = Some(self.active_tab_index);
        self.renaming_focus_pending = true;
    }

    fn close_active_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }

        self.tabs.remove(self.active_tab_index);
        self.active_tab_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        self.renaming_tab_index = None;
        self.renaming_focus_pending = false;
    }

    fn active_tab_name(&self) -> &str {
        self.active_tab().name.as_str()
    }

    fn active_tab(&self) -> &QuickDockTab {
        &self.tabs[self.active_tab_index.min(self.tabs.len().saturating_sub(1))]
    }

    fn active_tab_mut(&mut self) -> &mut QuickDockTab {
        let index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        &mut self.tabs[index]
    }

    fn open_settings_editor(&mut self) {
        self.active_tab_mut().editable_items = self
            .active_tab()
            .items
            .iter()
            .map(EditableActionItem::from_action_item)
            .collect();
        self.is_settings_editor_open = true;
        self.editor_focus_index = None;
        self.last_status_message = "설정을 편집 중입니다.".to_owned();
    }

    fn save_settings_editor(&mut self) {
        if let Some(error) = validate_editable_items_for_save(&self.active_tab().editable_items) {
            self.last_status_message = error;
            self.show_toast("설정을 확인하세요", true);
            return;
        }

        let items: Vec<ActionItem> = self
            .active_tab()
            .editable_items
            .iter()
            .map(EditableActionItem::to_action_item)
            .collect();

        let active_index = self.active_tab_index;
        self.tabs[active_index].items = items.clone();

        let layout = self.current_layout();
        match save_tabs_to_configuration(&self.configuration_path, &self.tabs, &layout, true) {
            Ok(()) => {
                self.last_status_message =
                    "설정을 저장했습니다. 이전 파일은 quick_dock.ini.bak에 백업했습니다."
                        .to_owned();
                self.is_settings_editor_open = false;
                self.editor_focus_index = None;
                self.last_hovered_at = Instant::now();
            }
            Err(error) => {
                self.last_status_message = format!("설정 저장 실패: {error}");
            }
        }
    }

    fn cancel_settings_editor(&mut self) {
        self.active_tab_mut().editable_items = self
            .active_tab()
            .items
            .iter()
            .map(EditableActionItem::from_action_item)
            .collect();
        self.is_settings_editor_open = false;
        self.editor_focus_index = None;
        self.last_hovered_at = Instant::now();
        self.last_status_message = "설정 편집을 취소했습니다.".to_owned();
    }

    fn add_editable_item(&mut self, kind: ActionKind) {
        self.active_tab_mut()
            .editable_items
            .push(EditableActionItem::blank(kind));
    }

    fn close_related_explorer_windows(&mut self) {
        match run_explorer_cleanup() {
            Ok(()) => {
                self.last_status_message = "중복/하위 탐색기 창을 정리했습니다.".to_owned();
            }
            Err(error) => {
                self.last_status_message = format!("탐색기 정리 실패: {error}");
            }
        }
    }

    fn show_collapsed_user_interface(&mut self, ui: &mut egui::Ui) {
        let available_rect = ui.max_rect();
        let response = ui.interact(
            available_rect,
            ui.id().with("collapsed_drag_handle"),
            egui::Sense::click(),
        );

        let painter = ui.painter();
        let is_vertical = matches!(self.dock_edge, DockEdge::Left | DockEdge::Right);
        let tab_rect = if is_vertical {
            available_rect.shrink2(egui::vec2(3.0, 8.0))
        } else {
            available_rect.shrink2(egui::vec2(8.0, 3.0))
        };
        let background = if response.hovered() {
            egui::Color32::from_rgb(19, 31, 39)
        } else {
            egui::Color32::from_rgb(12, 22, 29)
        };

        painter.rect_filled(
            tab_rect.translate(egui::vec2(0.0, 2.0)),
            13.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 72),
        );
        painter.rect_filled(tab_rect, 13.0, background);
        painter.rect_stroke(
            tab_rect,
            13.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 114, 137)),
            egui::StrokeKind::Inside,
        );

        let accent = if response.hovered() {
            egui::Color32::from_rgb(103, 232, 205)
        } else {
            egui::Color32::from_rgb(72, 189, 173)
        };
        let grip = egui::Color32::from_rgb(202, 224, 229);

        if is_vertical {
            let x = if self.dock_edge == DockEdge::Left {
                tab_rect.right() - 4.0
            } else {
                tab_rect.left() + 4.0
            };
            painter.line_segment(
                [
                    egui::pos2(x, tab_rect.top() + 18.0),
                    egui::pos2(x, tab_rect.bottom() - 18.0),
                ],
                egui::Stroke::new(2.0, accent),
            );

            for offset in [-10.0, 0.0, 10.0] {
                painter.line_segment(
                    [
                        tab_rect.center() + egui::vec2(-4.0, offset),
                        tab_rect.center() + egui::vec2(4.0, offset),
                    ],
                    egui::Stroke::new(1.7, grip),
                );
            }
        } else {
            let y = if self.dock_edge == DockEdge::Top {
                tab_rect.bottom() - 4.0
            } else {
                tab_rect.top() + 4.0
            };
            painter.line_segment(
                [
                    egui::pos2(tab_rect.left() + 18.0, y),
                    egui::pos2(tab_rect.right() - 18.0, y),
                ],
                egui::Stroke::new(2.0, accent),
            );

            for offset in [-10.0, 0.0, 10.0] {
                painter.line_segment(
                    [
                        tab_rect.center() + egui::vec2(offset, -4.0),
                        tab_rect.center() + egui::vec2(offset, 4.0),
                    ],
                    egui::Stroke::new(1.7, grip),
                );
            }
        }

        if response.clicked() {
            self.is_expanded = true;
            self.apply_dock_geometry(ui.ctx());
        }
    }

    fn show_drag_preview_user_interface(&mut self, ui: &mut egui::Ui) {
        let available_rect = ui.max_rect();
        let painter = ui.painter();

        if let Some(edge) = self.highlighted_dock_edge {
            let is_vertical = matches!(edge, DockEdge::Left | DockEdge::Right);
            let tab_rect = if is_vertical {
                available_rect.shrink2(egui::vec2(3.0, 8.0))
            } else {
                available_rect.shrink2(egui::vec2(8.0, 3.0))
            };

            painter.rect_filled(
                tab_rect.translate(egui::vec2(0.0, 2.0)),
                14.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            );
            painter.rect_filled(
                tab_rect,
                14.0,
                egui::Color32::from_rgba_unmultiplied(9, 24, 31, 190),
            );
            painter.rect_stroke(
                tab_rect,
                14.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(109, 214, 204)),
                egui::StrokeKind::Inside,
            );

            let accent = egui::Color32::from_rgb(103, 232, 205);
            let grip = egui::Color32::from_rgb(218, 236, 239);

            if is_vertical {
                let x = if edge == DockEdge::Left {
                    tab_rect.right() - 4.0
                } else {
                    tab_rect.left() + 4.0
                };
                painter.line_segment(
                    [
                        egui::pos2(x, tab_rect.top() + 15.0),
                        egui::pos2(x, tab_rect.bottom() - 15.0),
                    ],
                    egui::Stroke::new(1.8, accent),
                );

                for offset in [-8.0, 0.0, 8.0] {
                    painter.line_segment(
                        [
                            tab_rect.center() + egui::vec2(-4.0, offset),
                            tab_rect.center() + egui::vec2(4.0, offset),
                        ],
                        egui::Stroke::new(1.4, grip),
                    );
                }
            } else {
                let y = if edge == DockEdge::Top {
                    tab_rect.bottom() - 4.0
                } else {
                    tab_rect.top() + 4.0
                };
                painter.line_segment(
                    [
                        egui::pos2(tab_rect.left() + 15.0, y),
                        egui::pos2(tab_rect.right() - 15.0, y),
                    ],
                    egui::Stroke::new(1.8, accent),
                );

                for offset in [-8.0, 0.0, 8.0] {
                    painter.line_segment(
                        [
                            tab_rect.center() + egui::vec2(offset, -4.0),
                            tab_rect.center() + egui::vec2(offset, 4.0),
                        ],
                        egui::Stroke::new(1.4, grip),
                    );
                }
            }
        } else {
            let panel_rect = available_rect.shrink(6.0);
            let title_rect = egui::Rect::from_min_size(
                panel_rect.min,
                egui::vec2(panel_rect.width(), TITLE_BAR_HEIGHT),
            );

            painter.rect_filled(
                panel_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(242, 246, 248, 190),
            );
            painter.rect_stroke(
                panel_rect,
                8.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(125, 155, 171)),
                egui::StrokeKind::Inside,
            );
            painter.rect_filled(
                title_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(24, 40, 50, 175),
            );
            painter.text(
                title_rect.left_center() + egui::vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "Quick Dock",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            painter.text(
                title_rect.right_center() - egui::vec2(14.0, 0.0),
                egui::Align2::CENTER_CENTER,
                "X",
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );

            let line_color = egui::Color32::from_rgba_unmultiplied(57, 82, 96, 135);
            for row in 0..5 {
                let y = title_rect.bottom() + 28.0 + row as f32 * 34.0;
                painter.line_segment(
                    [
                        egui::pos2(panel_rect.left() + 22.0, y),
                        egui::pos2(panel_rect.right() - 22.0, y),
                    ],
                    egui::Stroke::new(1.0, line_color),
                );
            }
        }
    }

    fn show_expanded_user_interface(&mut self, ui: &mut egui::Ui) {
        let resize_rect = ui.max_rect();
        egui::Frame::default()
            .fill(egui::Color32::from_rgb(242, 246, 248))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(185, 197, 205),
            ))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                self.show_header(ui);
                ui.add_space(6.0);
                self.show_tab_strip(ui);
                ui.add_space(8.0);

                if self.is_settings_editor_open {
                    self.show_settings_editor(ui);
                } else if self.active_tab().items.is_empty() {
                    ui.label("env\\quick_dock.ini에 등록된 항목이 없습니다.");
                } else {
                    let pending_request = egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let items = self.active_tab().items.clone();
                            let item_count = items.len();
                            let mut pending = None;
                            for (index, item) in items.iter().enumerate() {
                                if let Some(request) =
                                    self.show_action_button(ui, index, item_count, item)
                                {
                                    pending = Some(request);
                                }
                                ui.add_space(4.0);
                            }
                            pending
                        })
                        .inner;

                    if let Some(request) = pending_request {
                        self.handle_button_action_request(request);
                    }
                }

                ui.separator();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(self.active_tab_name()).strong());
                    ui.label(shorten_text(&self.last_status_message, 46));
                });
            });

        self.show_expanded_resize_handles(ui, resize_rect);
    }

    fn show_expanded_resize_handles(&mut self, ui: &mut egui::Ui, frame_rect: egui::Rect) {
        if !self.is_expanded || self.is_dragging {
            return;
        }

        let is_docked = self.is_docked;
        let context = ui.ctx().clone();
        for (edge, rect) in resize_handle_rects(frame_rect, self.dock_edge) {
            let response = ui.interact(
                rect,
                ui.id().with(("expanded_resize", edge.id_salt())),
                egui::Sense::click_and_drag(),
            );

            if response.hovered() || response.dragged() {
                ui.output_mut(|output| output.cursor_icon = edge.cursor_icon());
            }

            if response.drag_started_by(egui::PointerButton::Primary) {
                if is_docked {
                    self.begin_expanded_resize(edge, &context);
                } else {
                    self.begin_window_resize(edge, &context);
                }
            }
        }
    }

    fn show_action_button(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        item_count: usize,
        item: &ActionItem,
    ) -> Option<ButtonActionRequest> {
        let mut request = None;
        let button_label = format_action_button_label(item);
        let response = ui.add_sized(
            [ui.available_width(), 34.0],
            egui::Button::new(button_label),
        );

        let response = if let ActionItem::CopyText { name, text } = item {
            response.on_hover_ui(|ui| show_copy_text_preview(ui, name, text))
        } else {
            response
        };

        response.context_menu(|ui| {
            ui.set_min_width(160.0);

            match item {
                ActionItem::CopyText { name, text } => {
                    show_copy_text_preview(ui, name, text);
                    ui.separator();
                    if ui.button("복사").clicked() {
                        request = Some(ButtonActionRequest::Execute(index));
                        ui.close();
                    }
                }
                ActionItem::RunApplication { .. } => {
                    if ui.button("실행").clicked() {
                        request = Some(ButtonActionRequest::Execute(index));
                        ui.close();
                    }
                    if ui.button("관리자 권한으로 실행").clicked() {
                        request = Some(ButtonActionRequest::RunAsAdmin(index));
                        ui.close();
                    }
                    if ui.button("파일 위치 열기").clicked() {
                        request = Some(ButtonActionRequest::OpenItemLocation(index));
                        ui.close();
                    }
                    if ui.button("경로 복사").clicked() {
                        request = Some(ButtonActionRequest::CopyItemPath(index));
                        ui.close();
                    }
                }
                ActionItem::OpenPath { .. } => {
                    if ui.button("열기").clicked() {
                        request = Some(ButtonActionRequest::Execute(index));
                        ui.close();
                    }
                    if ui.button("탐색기에서 선택").clicked() {
                        request = Some(ButtonActionRequest::OpenItemLocation(index));
                        ui.close();
                    }
                    if ui.button("경로 복사").clicked() {
                        request = Some(ButtonActionRequest::CopyItemPath(index));
                        ui.close();
                    }
                }
            }

            ui.separator();
            if ui.button("편집").clicked() {
                request = Some(ButtonActionRequest::EditItem(index));
                ui.close();
            }
            if ui.button("복제").clicked() {
                request = Some(ButtonActionRequest::DuplicateItem(index));
                ui.close();
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("위로"))
                .clicked()
            {
                request = Some(ButtonActionRequest::MoveUp(index));
                ui.close();
            }
            if ui
                .add_enabled(index + 1 < item_count, egui::Button::new("아래로"))
                .clicked()
            {
                request = Some(ButtonActionRequest::MoveDown(index));
                ui.close();
            }
            if ui.button("삭제").clicked() {
                request = Some(ButtonActionRequest::Remove(index));
                ui.close();
            }
        });

        if response.clicked() {
            request = Some(ButtonActionRequest::Execute(index));
        }

        request
    }

    fn handle_button_action_request(&mut self, request: ButtonActionRequest) {
        let item_index = match request {
            ButtonActionRequest::Execute(index)
            | ButtonActionRequest::RunAsAdmin(index)
            | ButtonActionRequest::OpenItemLocation(index)
            | ButtonActionRequest::CopyItemPath(index)
            | ButtonActionRequest::EditItem(index)
            | ButtonActionRequest::DuplicateItem(index)
            | ButtonActionRequest::MoveUp(index)
            | ButtonActionRequest::MoveDown(index)
            | ButtonActionRequest::Remove(index) => index,
        };

        let Some(item) = self.active_tab().items.get(item_index).cloned() else {
            return;
        };

        match request {
            ButtonActionRequest::Execute(_) => {
                self.record_recent(&item);
                self.execute_action(&item);
            }
            ButtonActionRequest::RunAsAdmin(_) => self.run_item_as_admin(&item),
            ButtonActionRequest::OpenItemLocation(_) => self.open_item_location(&item),
            ButtonActionRequest::CopyItemPath(_) => self.copy_item_path(&item),
            ButtonActionRequest::EditItem(_) => self.open_settings_editor_at(item_index),
            ButtonActionRequest::DuplicateItem(_) => {
                let active_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
                self.tabs[active_index]
                    .items
                    .insert(item_index + 1, item.clone());
                self.commit_active_items(format!("복제했습니다: {}", item.name()));
            }
            ButtonActionRequest::MoveUp(_) => {
                if item_index > 0 {
                    let active_index =
                        self.active_tab_index.min(self.tabs.len().saturating_sub(1));
                    self.tabs[active_index].items.swap(item_index, item_index - 1);
                    self.commit_active_items("순서를 변경했습니다.");
                }
            }
            ButtonActionRequest::MoveDown(_) => {
                let active_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
                if item_index + 1 < self.tabs[active_index].items.len() {
                    self.tabs[active_index].items.swap(item_index, item_index + 1);
                    self.commit_active_items("순서를 변경했습니다.");
                }
            }
            ButtonActionRequest::Remove(_) => {
                let active_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
                self.tabs[active_index].items.remove(item_index);
                self.commit_active_items(format!("삭제했습니다: {}", item.name()));
            }
        }
    }

    fn commit_active_items(&mut self, status: impl Into<String>) {
        let active_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        self.tabs[active_index].editable_items = self.tabs[active_index]
            .items
            .iter()
            .map(EditableActionItem::from_action_item)
            .collect();

        let layout = self.current_layout();
        match save_tabs_to_configuration(&self.configuration_path, &self.tabs, &layout, true) {
            Ok(()) => self.last_status_message = status.into(),
            Err(error) => {
                self.last_status_message = format!("저장 실패: {error}");
                self.show_toast("저장 실패", true);
            }
        }
    }

    fn open_settings_editor_at(&mut self, index: usize) {
        self.open_settings_editor();
        self.editor_focus_index = Some(index);
        self.last_status_message = format!("{}번째 항목을 편집합니다.", index + 1);
    }

    fn run_item_as_admin(&mut self, item: &ActionItem) {
        let ActionItem::RunApplication {
            name,
            command,
            arguments,
        } = item
        else {
            return;
        };

        match run_application_as_admin(command, arguments) {
            Ok(()) => {
                self.last_status_message = format!("관리자 권한으로 실행: {name}");
                self.show_toast("관리자 권한으로 실행", false);
            }
            Err(message) => {
                self.last_status_message = format!("관리자 실행 실패: {message}");
                self.show_toast("관리자 실행 실패", true);
            }
        }
    }

    fn open_item_location(&mut self, item: &ActionItem) {
        let target = match item {
            ActionItem::RunApplication { command, .. } => {
                resolve_application_command_for_spawn(command)
            }
            ActionItem::OpenPath { path, .. } => resolve_open_path_for_spawn(path),
            ActionItem::CopyText { .. } => return,
        };

        match target.and_then(|path| reveal_in_file_explorer(&path)) {
            Ok(()) => {
                self.last_status_message = "파일 위치를 열었습니다.".to_owned();
                self.show_toast("파일 위치 열기", false);
            }
            Err(message) => {
                self.last_status_message = format!("파일 위치 열기 실패: {message}");
                self.show_toast("파일 위치 열기 실패", true);
            }
        }
    }

    fn copy_item_path(&mut self, item: &ActionItem) {
        let path_text = match item {
            ActionItem::RunApplication { command, .. } => {
                resolve_application_command_for_spawn(command)
                    .unwrap_or_else(|_| normalize_command_text(command))
            }
            ActionItem::OpenPath { path, .. } => normalize_command_text(path),
            ActionItem::CopyText { .. } => return,
        };

        self.set_clipboard_text("경로", &path_text);
    }

    fn show_header(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                let close_size = egui::vec2(30.0, TITLE_BAR_HEIGHT);
                let title_width = (ui.available_width() - close_size.x - 4.0).max(80.0);
                let (title_rect, title_response) = ui.allocate_exact_size(
                    egui::vec2(title_width, TITLE_BAR_HEIGHT),
                    egui::Sense::click_and_drag(),
                );

                let title_fill = if title_response.dragged() {
                    egui::Color32::from_rgb(224, 234, 238)
                } else if title_response.hovered() {
                    egui::Color32::from_rgb(232, 240, 244)
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.painter().rect_filled(title_rect, 4.0, title_fill);
                ui.painter().text(
                    title_rect.left_center() + egui::vec2(6.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Quick Dock",
                    egui::FontId::proportional(17.0),
                    egui::Color32::from_rgb(22, 35, 43),
                );

                if title_response.hovered() {
                    ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::Grab);
                }

                if title_response.drag_started_by(egui::PointerButton::Primary) {
                    self.begin_title_bar_drag(ui.ctx());
                }

                let close_response = ui.add_sized(
                    close_size,
                    egui::Button::new(egui::RichText::new("X").strong().size(14.0))
                        .fill(egui::Color32::TRANSPARENT),
                );

                if close_response.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.horizontal_wrapped(|ui| {
                if ui.small_button("검색").clicked() {
                    self.toggle_palette();
                }

                if ui.small_button("새 탭").clicked() {
                    self.add_tab();
                }

                if ui.small_button("다시 읽기").clicked() {
                    self.reload_configuration();
                }

                if ui.small_button("탐색기 정리").clicked() {
                    self.close_related_explorer_windows();
                }

                let autostart_label = if self.autostart_enabled {
                    "자동시작 ✓"
                } else {
                    "자동시작"
                };
                if ui.small_button(autostart_label).clicked() {
                    self.toggle_autostart();
                }

                if ui.small_button("설정").clicked() {
                    if self.is_settings_editor_open {
                        self.cancel_settings_editor();
                    } else {
                        self.open_settings_editor();
                    }
                }
            });
        });
    }

    fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut tab_to_close = None;
            let mut tab_to_rename = None;
            let mut finish_renaming = false;

            for index in 0..self.tabs.len() {
                let is_active = index == self.active_tab_index;
                let is_renaming = self.renaming_tab_index == Some(index);

                let response = if is_renaming {
                    let response = ui.add_sized(
                        [110.0, 24.0],
                        egui::TextEdit::singleline(&mut self.tabs[index].name).desired_width(110.0),
                    );
                    if self.renaming_focus_pending {
                        response.request_focus();
                        self.renaming_focus_pending = false;
                    }
                    response
                } else {
                    let tab_name = self.tabs[index].name.clone();
                    ui.selectable_label(is_active, tab_name)
                };

                if response.clicked() {
                    self.active_tab_index = index;
                    if !is_renaming {
                        self.renaming_tab_index = None;
                    }
                }

                if is_renaming {
                    let pressed_enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
                    let pressed_escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
                    let clicked_elsewhere =
                        response.lost_focus() && ui.input(|input| input.pointer.any_pressed());

                    if pressed_enter || pressed_escape || clicked_elsewhere {
                        finish_renaming = true;
                    }
                }

                response.context_menu(|ui| {
                    ui.set_min_width(120.0);
                    if ui.button("이름 변경").clicked() {
                        tab_to_rename = Some(index);
                        ui.close();
                    }

                    if self.tabs.len() > 1 && ui.button("탭 닫기").clicked() {
                        tab_to_close = Some(index);
                        ui.close();
                    }
                });
            }

            if finish_renaming {
                if let Some(index) = self.renaming_tab_index {
                    if self.tabs[index].name.trim().is_empty() {
                        self.tabs[index].name = format!("탭 {}", index + 1);
                    }
                }
                self.renaming_tab_index = None;
                self.renaming_focus_pending = false;
            }

            if let Some(index) = tab_to_rename {
                self.active_tab_index = index;
                self.renaming_tab_index = Some(index);
                self.renaming_focus_pending = true;
            }

            if let Some(index) = tab_to_close {
                self.active_tab_index = index;
                self.close_active_tab();
            }
        });
    }

    fn show_settings_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("저장").clicked() {
                self.save_settings_editor();
            }

            if ui.button("취소").clicked() {
                self.cancel_settings_editor();
            }

            ui.separator();

            egui::ComboBox::from_id_salt("new_item_kind")
                .selected_text(self.new_item_kind.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.new_item_kind, ActionKind::CopyText, "복사");
                    ui.selectable_value(
                        &mut self.new_item_kind,
                        ActionKind::RunApplication,
                        "실행",
                    );
                    ui.selectable_value(&mut self.new_item_kind, ActionKind::OpenPath, "열기");
                });

            if ui.small_button("추가").clicked() {
                self.add_editable_item(self.new_item_kind);
            }
        });

        ui.add_space(8.0);

        let mut remove_index = None;
        let mut move_up_index = None;
        let mut move_down_index = None;
        let mut editor_action_request = None;
        let item_count = self.active_tab().editable_items.len();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for index in 0..self.active_tab().editable_items.len() {
                    let title = format!(
                        "{} · {}",
                        self.active_tab().editable_items[index].kind.label(),
                        shorten_text(&self.active_tab().editable_items[index].name, 24)
                    );

                    let header = egui::CollapsingHeader::new(title)
                        .id_salt(("setting_item", index));
                    let header = match self.editor_focus_index {
                        Some(focus) if focus == index => header.open(Some(true)),
                        _ => header.default_open(index == 0),
                    };
                    let response = header
                        .show(ui, |ui| {
                            let item = &mut self.active_tab_mut().editable_items[index];
                            if let Some(request) = show_editable_action_item(ui, index, item) {
                                editor_action_request = Some(request);
                            }
                        });

                    response.header_response.context_menu(|ui| {
                        if ui
                            .add_enabled(index > 0, egui::Button::new("위로"))
                            .clicked()
                        {
                            move_up_index = Some(index);
                            ui.close();
                        }

                        if ui
                            .add_enabled(index + 1 < item_count, egui::Button::new("아래로"))
                            .clicked()
                        {
                            move_down_index = Some(index);
                            ui.close();
                        }

                        if ui.button("삭제").clicked() {
                            remove_index = Some(index);
                            ui.close();
                        }
                    });

                    ui.add_space(4.0);
                }
            });

        if let Some(index) = move_up_index {
            self.active_tab_mut().editable_items.swap(index, index - 1);
        }

        if let Some(index) = move_down_index {
            self.active_tab_mut().editable_items.swap(index, index + 1);
        }

        if let Some(index) = remove_index {
            self.active_tab_mut().editable_items.remove(index);
        }

        if let Some(request) = editor_action_request {
            match request {
                EditorActionRequest::Test(item) => {
                    self.execute_action(&item);
                }
                EditorActionRequest::Status { message, is_error } => {
                    self.last_status_message = message.clone();
                    self.show_toast(message, is_error);
                }
            }
        }
    }

    fn pointer_to_monitor_position(&self, pointer_position: egui::Pos2) -> egui::Pos2 {
        self.last_known_inner_position + pointer_position.to_vec2()
    }

    fn execute_action(&mut self, item: &ActionItem) {
        match item {
            ActionItem::CopyText { name, text } => {
                self.begin_copy(name, text);
            }
            ActionItem::RunApplication {
                name,
                command,
                arguments,
            } => {
                self.run_application(name, command, arguments);
            }
            ActionItem::OpenPath { name, path } => {
                self.open_path(name, path);
            }
        }
    }

    fn begin_copy(&mut self, name: &str, text: &str) {
        let labels = extract_input_labels(text);
        if labels.is_empty() {
            let selection_text = self.selection_text_for_template(text);
            let expanded = expand_copy_template(text, &[], &selection_text);
            self.set_clipboard_text(name, &expanded);
            return;
        }

        self.pending_input_copy = Some(PendingInputCopy {
            name: name.to_owned(),
            template: text.to_owned(),
            fields: labels
                .into_iter()
                .map(|label| TemplateInputField {
                    label,
                    value: String::new(),
                })
                .collect(),
            focus_pending: true,
        });
        self.last_status_message = "값을 입력한 뒤 복사합니다.".to_owned();
    }

    fn confirm_pending_input_copy(&mut self) {
        let Some(pending) = self.pending_input_copy.take() else {
            return;
        };
        let selection_text = self.selection_text_for_template(&pending.template);
        let expanded = expand_copy_template(&pending.template, &pending.fields, &selection_text);
        self.set_clipboard_text(&pending.name, &expanded);
    }

    fn selection_text_for_template(&self, template: &str) -> String {
        if template.contains("{selection}") {
            self.capture_selection_text().unwrap_or_default()
        } else {
            String::new()
        }
    }

    fn capture_selection_text(&self) -> Option<String> {
        capture_selection_text(self.last_external_foreground_window)
    }

    fn set_clipboard_text(&mut self, name: &str, text: &str) {
        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text.to_owned())) {
            Ok(()) => {
                self.last_status_message = format!("복사되었습니다: {name}");
                self.show_toast("복사되었습니다", false);
            }
            Err(error) => {
                self.last_status_message = format!("복사 실패: {error}");
                self.show_toast("복사 실패", true);
            }
        }
    }

    fn show_pending_input_modal(&mut self, context: &egui::Context) {
        if self.pending_input_copy.is_none() {
            return;
        }

        context.request_repaint_after(Duration::from_millis(80));

        // 모달이 열린 첫 프레임에는 Enter를 무시한다.
        // (팔레트에서 Enter로 연 경우 같은 Enter가 모달을 즉시 확정하는 것을 막는다.)
        let just_opened = self
            .pending_input_copy
            .as_ref()
            .map(|pending| pending.focus_pending)
            .unwrap_or(false);

        let mut confirm = false;
        let mut cancel = false;

        egui::Area::new(egui::Id::new("quick_dock_input_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(context, |ui| {
                let backdrop_rect = ui.ctx().content_rect();
                ui.painter().rect_filled(
                    backdrop_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120),
                );
                ui.allocate_rect(backdrop_rect, egui::Sense::click_and_drag());
            });

        let pending_name = self
            .pending_input_copy
            .as_ref()
            .map(|pending| pending.name.clone())
            .unwrap_or_default();

        egui::Window::new(format!("입력: {}", shorten_text(&pending_name, 18)))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(context, |ui| {
                ui.set_min_width(240.0);
                if let Some(pending) = self.pending_input_copy.as_mut() {
                    for (index, field) in pending.fields.iter_mut().enumerate() {
                        ui.label(egui::RichText::new(&field.label).strong());
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut field.value)
                                .desired_width(f32::INFINITY)
                                .hint_text("값 입력"),
                        );
                        if pending.focus_pending && index == 0 {
                            response.request_focus();
                            pending.focus_pending = false;
                        }
                        ui.add_space(6.0);
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("복사").clicked() {
                        confirm = true;
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                    ui.label(
                        egui::RichText::new("Enter 복사 · Esc 취소")
                            .small()
                            .weak(),
                    );
                });
            });

        if !just_opened && context.input(|input| input.key_pressed(egui::Key::Enter)) {
            confirm = true;
        }
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }

        if confirm {
            self.confirm_pending_input_copy();
        } else if cancel {
            self.pending_input_copy = None;
            self.last_status_message = "복사를 취소했습니다.".to_owned();
        }
    }

    fn ensure_tray_icon(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if self.tray_state.is_some() || self.tray_attempted {
                return;
            }
            self.tray_attempted = true;
            match build_tray_state(self.autostart_enabled) {
                Ok(state) => self.tray_state = Some(state),
                Err(error) => log_event(&format!("트레이 아이콘 생성 실패: {error}")),
            }
        }
    }

    fn poll_tray_events(&mut self, context: &egui::Context) {
        #[cfg(target_os = "windows")]
        {
            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                if event.id == "tray_open" {
                    self.bring_to_front(context);
                } else if event.id == "tray_settings" {
                    self.bring_to_front(context);
                    if !self.is_settings_editor_open {
                        self.open_settings_editor();
                    }
                } else if event.id == "tray_autostart" {
                    self.toggle_autostart();
                    if let Some(state) = &self.tray_state {
                        state.autostart_item.set_checked(self.autostart_enabled);
                    }
                } else if event.id == "tray_quit" {
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }

            while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if let tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    ..
                } = event
                {
                    self.bring_to_front(context);
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        let _ = context;
    }

    fn bring_to_front(&mut self, context: &egui::Context) {
        self.is_expanded = true;
        if self.is_docked {
            self.apply_dock_geometry(context);
        }
        context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        context.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.last_hovered_at = Instant::now();
    }

    fn toggle_autostart(&mut self) {
        let target = !self.autostart_enabled;
        match set_autostart(target) {
            Ok(()) => {
                self.autostart_enabled = target;
                let message = if target {
                    "Windows 시작 시 자동 실행을 켰습니다."
                } else {
                    "자동 실행을 껐습니다."
                };
                self.last_status_message = message.to_owned();
                self.show_toast(message, false);
                log_event(&format!("자동 실행 설정: {target}"));
            }
            Err(error) => {
                self.last_status_message = format!("자동 실행 설정 실패: {error}");
                self.show_toast("자동 실행 설정 실패", true);
                log_event(&format!("자동 실행 설정 실패: {error}"));
            }
        }
    }

    fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
        if self.palette_open {
            self.palette_query.clear();
            self.palette_selected = 0;
            self.palette_focus_pending = true;
            self.last_status_message = "검색: 이름·내용·탭".to_owned();
        }
    }

    fn record_recent(&mut self, item: &ActionItem) {
        self.recent_items
            .retain(|existing| existing.kind() != item.kind() || existing.name() != item.name());
        self.recent_items.insert(0, item.clone());
        self.recent_items.truncate(8);
    }

    fn build_palette_entries(&self) -> Vec<PaletteEntry> {
        let query = self.palette_query.trim().to_lowercase();
        let mut entries = Vec::new();

        if query.is_empty() {
            for item in &self.recent_items {
                entries.push(PaletteEntry {
                    item: item.clone(),
                    tab_index: None,
                    tab_label: "최근".to_owned(),
                    is_recent: true,
                });
            }
            for (tab_index, tab) in self.tabs.iter().enumerate() {
                for item in &tab.items {
                    entries.push(PaletteEntry {
                        item: item.clone(),
                        tab_index: Some(tab_index),
                        tab_label: tab.name.clone(),
                        is_recent: false,
                    });
                }
            }
        } else {
            for (tab_index, tab) in self.tabs.iter().enumerate() {
                for item in &tab.items {
                    let haystack = format!("{} {} {}", tab.name, item.name(), item.search_payload())
                        .to_lowercase();
                    if haystack.contains(&query) {
                        entries.push(PaletteEntry {
                            item: item.clone(),
                            tab_index: Some(tab_index),
                            tab_label: tab.name.clone(),
                            is_recent: false,
                        });
                    }
                }
            }
        }

        entries.truncate(80);
        entries
    }

    fn show_palette(&mut self, context: &egui::Context) {
        if !self.palette_open {
            return;
        }

        context.request_repaint_after(Duration::from_millis(80));

        let entries = self.build_palette_entries();
        let entry_count = entries.len();
        self.palette_selected = if entry_count == 0 {
            0
        } else {
            self.palette_selected.min(entry_count - 1)
        };

        let mut execute_now = false;
        let mut close_now = false;
        context.input(|input| {
            if entry_count > 0 && input.key_pressed(egui::Key::ArrowDown) {
                self.palette_selected = (self.palette_selected + 1) % entry_count;
            }
            if entry_count > 0 && input.key_pressed(egui::Key::ArrowUp) {
                self.palette_selected = (self.palette_selected + entry_count - 1) % entry_count;
            }
            if input.key_pressed(egui::Key::Enter) {
                execute_now = true;
            }
            if input.key_pressed(egui::Key::Escape) {
                close_now = true;
            }
        });

        egui::Area::new(egui::Id::new("quick_dock_palette_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(context, |ui| {
                let backdrop_rect = ui.ctx().content_rect();
                ui.painter().rect_filled(
                    backdrop_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120),
                );
                if ui
                    .allocate_rect(backdrop_rect, egui::Sense::click())
                    .clicked()
                {
                    close_now = true;
                }
            });

        let mut clicked_index = None;
        egui::Window::new("검색")
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 44.0))
            .show(context, |ui| {
                ui.set_min_width(280.0);
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text("이름·내용·탭 검색")
                        .desired_width(f32::INFINITY),
                );
                if self.palette_focus_pending {
                    response.request_focus();
                    self.palette_focus_pending = false;
                }

                ui.separator();

                if entries.is_empty() {
                    ui.label("일치하는 항목이 없습니다.");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (index, entry) in entries.iter().enumerate() {
                                let prefix = if entry.is_recent { "★ " } else { "" };
                                let label = format!(
                                    "{prefix}{} · {}",
                                    format_action_button_label(&entry.item),
                                    entry.tab_label
                                );
                                let response = ui.selectable_label(
                                    index == self.palette_selected,
                                    shorten_text(&label, 52),
                                );
                                if response.clicked() {
                                    clicked_index = Some(index);
                                }
                            }
                        });
                }

                ui.separator();
                ui.label(
                    egui::RichText::new("↑↓ 이동 · Enter 실행 · Esc 닫기")
                        .small()
                        .weak(),
                );
            });

        if let Some(index) = clicked_index {
            self.palette_selected = index;
            execute_now = true;
        }

        if close_now {
            self.palette_open = false;
            return;
        }

        if execute_now {
            if let Some(entry) = entries.into_iter().nth(self.palette_selected) {
                self.execute_palette_entry(entry);
            }
        }
    }

    fn execute_palette_entry(&mut self, entry: PaletteEntry) {
        self.palette_open = false;
        if let Some(tab_index) = entry.tab_index {
            self.active_tab_index = tab_index.min(self.tabs.len().saturating_sub(1));
        }
        self.record_recent(&entry.item);
        self.execute_action(&entry.item);
    }

    fn show_toast(&mut self, message: impl Into<String>, is_error: bool) {
        self.toast_message = Some(message.into());
        self.toast_started_at = Instant::now();
        self.toast_is_error = is_error;
    }

    fn show_toast_overlay(&mut self, context: &egui::Context) {
        let Some(message) = self.toast_message.clone() else {
            return;
        };

        if self.toast_started_at.elapsed() >= Duration::from_millis(TOAST_DURATION_MILLISECONDS) {
            self.toast_message = None;
            return;
        }

        context.request_repaint_after(Duration::from_millis(80));

        let fill = if self.toast_is_error {
            egui::Color32::from_rgb(188, 55, 68)
        } else {
            egui::Color32::from_rgb(28, 142, 86)
        };

        egui::Area::new(egui::Id::new("quick_dock_toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 12.0))
            .show(context, |ui| {
                egui::Frame::default()
                    .fill(fill)
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                    ))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(message)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                    });
            });
    }

    fn run_application(&mut self, name: &str, command: &str, arguments: &[String]) {
        let resolved_command = match resolve_application_command_for_spawn(command) {
            Ok(resolved_command) => resolved_command,
            Err(message) => {
                self.last_status_message = format!("실행 실패: {message}");
                self.show_toast("실행 실패", true);
                log_event(&format!("실행 실패 [{name}] command='{command}': {message}"));
                return;
            }
        };
        let expanded_arguments: Vec<String> = arguments
            .iter()
            .map(|argument| expand_environment_variables(argument))
            .collect();

        match Command::new(&resolved_command)
            .args(expanded_arguments)
            .spawn()
        {
            Ok(_) => {
                self.last_status_message = format!("실행 완료: {name}");
                self.show_toast("실행했습니다", false);
            }
            Err(error) => {
                let detail = describe_spawn_error(&error, &resolved_command);
                self.last_status_message = format!("실행 실패: {detail}");
                self.show_toast("실행 실패", true);
                log_event(&format!("실행 실패 [{name}]: {detail}"));
            }
        }
    }

    fn open_path(&mut self, name: &str, path: &str) {
        let expanded_path = match resolve_open_path_for_spawn(path) {
            Ok(expanded_path) => expanded_path,
            Err(message) => {
                self.last_status_message = format!("열기 실패: {message}");
                self.show_toast("열기 실패", true);
                log_event(&format!("열기 실패 [{name}] path='{path}': {message}"));
                return;
            }
        };

        #[cfg(target_os = "windows")]
        let result = Command::new("explorer.exe").arg(expanded_path).spawn();

        #[cfg(target_os = "macos")]
        let result = Command::new("open").arg(expanded_path).spawn();

        #[cfg(all(unix, not(target_os = "macos")))]
        let result = Command::new("xdg-open").arg(expanded_path).spawn();

        match result {
            Ok(_) => {
                self.last_status_message = format!("열기 완료: {name}");
                self.show_toast("열었습니다", false);
            }
            Err(error) => {
                let detail = describe_spawn_error(&error, path);
                self.last_status_message = format!("열기 실패: {detail}");
                self.show_toast("열기 실패", true);
                log_event(&format!("열기 실패 [{name}]: {detail}"));
            }
        }
    }
}

impl eframe::App for QuickDockApplication {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.is_docked || self.is_dragging {
            egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
        } else {
            egui::Color32::from_rgb(242, 246, 248).to_normalized_gamma_f32()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.ensure_tray_icon();
        self.poll_tray_events(&context);
        self.update_window_state(&context, frame);

        if context.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Space)) {
            self.toggle_palette();
        }

        if self.is_dragging {
            let _ = frame;
            self.show_drag_preview_user_interface(ui);
        } else if self.is_expanded {
            let _ = frame;
            self.show_expanded_user_interface(ui);
        } else {
            self.show_collapsed_user_interface(ui);
        }

        self.show_palette(&context);
        self.show_pending_input_modal(&context);
        self.show_toast_overlay(&context);
    }
}

fn get_window_size(dock_edge: DockEdge, is_expanded: bool) -> egui::Vec2 {
    if is_expanded {
        return egui::vec2(EXPANDED_WIDTH, EXPANDED_HEIGHT);
    }

    match dock_edge {
        DockEdge::Left | DockEdge::Right => egui::vec2(COLLAPSED_THICKNESS, COLLAPSED_LENGTH),
        DockEdge::Top | DockEdge::Bottom => egui::vec2(COLLAPSED_LENGTH, COLLAPSED_THICKNESS),
    }
}

fn default_expanded_size() -> egui::Vec2 {
    egui::vec2(EXPANDED_WIDTH, EXPANDED_HEIGHT)
}

fn clamp_expanded_size(size: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        size.x.clamp(MIN_EXPANDED_WIDTH, MAX_EXPANDED_WIDTH),
        size.y.clamp(MIN_EXPANDED_HEIGHT, MAX_EXPANDED_HEIGHT),
    )
}

fn clamp_expanded_size_to_monitor(size: egui::Vec2, monitor_rect: egui::Rect) -> egui::Vec2 {
    let maximum_width = monitor_rect.width().max(MIN_EXPANDED_WIDTH);
    let maximum_height = monitor_rect.height().max(MIN_EXPANDED_HEIGHT);
    let maximum_size = egui::vec2(
        MAX_EXPANDED_WIDTH.min(maximum_width),
        MAX_EXPANDED_HEIGHT.min(maximum_height),
    );

    egui::vec2(
        size.x.clamp(MIN_EXPANDED_WIDTH, maximum_size.x),
        size.y.clamp(MIN_EXPANDED_HEIGHT, maximum_size.y),
    )
}

fn resize_handle_rects(
    window_rect: egui::Rect,
    _dock_edge: DockEdge,
) -> Vec<(ResizeEdge, egui::Rect)> {
    let corner = RESIZE_HANDLE_THICKNESS * 2.4;
    let left = window_rect.left();
    let right = window_rect.right();
    let top = window_rect.top();
    let bottom = window_rect.bottom();

    let top_left = egui::Rect::from_min_size(window_rect.left_top(), egui::vec2(corner, corner));
    let top_right = egui::Rect::from_min_max(
        egui::pos2(right - corner, top),
        egui::pos2(right, top + corner),
    );
    let bottom_left = egui::Rect::from_min_max(
        egui::pos2(left, bottom - corner),
        egui::pos2(left + corner, bottom),
    );
    let bottom_right = egui::Rect::from_min_max(
        egui::pos2(right - corner, bottom - corner),
        window_rect.right_bottom(),
    );

    vec![
        (ResizeEdge::TopLeft, top_left),
        (ResizeEdge::TopRight, top_right),
        (ResizeEdge::BottomLeft, bottom_left),
        (ResizeEdge::BottomRight, bottom_right),
    ]
}

fn clamp_resize_rect(
    mut rect: egui::Rect,
    resize_edge: ResizeEdge,
    dock_edge: DockEdge,
    monitor_rect: egui::Rect,
) -> egui::Rect {
    match dock_edge {
        DockEdge::Left => rect.min.x = monitor_rect.min.x,
        DockEdge::Right => rect.max.x = monitor_rect.max.x,
        DockEdge::Top => rect.min.y = monitor_rect.min.y,
        DockEdge::Bottom => rect.max.y = monitor_rect.max.y,
    }

    rect.min.x = rect.min.x.clamp(monitor_rect.min.x, monitor_rect.max.x);
    rect.max.x = rect.max.x.clamp(monitor_rect.min.x, monitor_rect.max.x);
    rect.min.y = rect.min.y.clamp(monitor_rect.min.y, monitor_rect.max.y);
    rect.max.y = rect.max.y.clamp(monitor_rect.min.y, monitor_rect.max.y);

    if rect.width() < MIN_EXPANDED_WIDTH {
        if resize_edge.affects_left() {
            rect.min.x = rect.max.x - MIN_EXPANDED_WIDTH;
        } else {
            rect.max.x = rect.min.x + MIN_EXPANDED_WIDTH;
        }
    }

    if rect.height() < MIN_EXPANDED_HEIGHT {
        if resize_edge.affects_top() {
            rect.min.y = rect.max.y - MIN_EXPANDED_HEIGHT;
        } else {
            rect.max.y = rect.min.y + MIN_EXPANDED_HEIGHT;
        }
    }

    if rect.width() > MAX_EXPANDED_WIDTH {
        if resize_edge.affects_left() {
            rect.min.x = rect.max.x - MAX_EXPANDED_WIDTH;
        } else {
            rect.max.x = rect.min.x + MAX_EXPANDED_WIDTH;
        }
    }

    if rect.height() > MAX_EXPANDED_HEIGHT {
        if resize_edge.affects_top() {
            rect.min.y = rect.max.y - MAX_EXPANDED_HEIGHT;
        } else {
            rect.max.y = rect.min.y + MAX_EXPANDED_HEIGHT;
        }
    }

    rect.min.x = rect.min.x.clamp(monitor_rect.min.x, monitor_rect.max.x);
    rect.max.x = rect.max.x.clamp(monitor_rect.min.x, monitor_rect.max.x);
    rect.min.y = rect.min.y.clamp(monitor_rect.min.y, monitor_rect.max.y);
    rect.max.y = rect.max.y.clamp(monitor_rect.min.y, monitor_rect.max.y);
    rect
}

fn clamp_free_window_rect(
    mut rect: egui::Rect,
    resize_edge: ResizeEdge,
    monitor_rect: egui::Rect,
) -> egui::Rect {
    if rect.width() < MIN_EXPANDED_WIDTH {
        if resize_edge.affects_left() {
            rect.min.x = rect.max.x - MIN_EXPANDED_WIDTH;
        } else {
            rect.max.x = rect.min.x + MIN_EXPANDED_WIDTH;
        }
    }
    if rect.width() > MAX_EXPANDED_WIDTH {
        if resize_edge.affects_left() {
            rect.min.x = rect.max.x - MAX_EXPANDED_WIDTH;
        } else {
            rect.max.x = rect.min.x + MAX_EXPANDED_WIDTH;
        }
    }
    if rect.height() < MIN_EXPANDED_HEIGHT {
        if resize_edge.affects_top() {
            rect.min.y = rect.max.y - MIN_EXPANDED_HEIGHT;
        } else {
            rect.max.y = rect.min.y + MIN_EXPANDED_HEIGHT;
        }
    }
    if rect.height() > MAX_EXPANDED_HEIGHT {
        if resize_edge.affects_top() {
            rect.min.y = rect.max.y - MAX_EXPANDED_HEIGHT;
        } else {
            rect.max.y = rect.min.y + MAX_EXPANDED_HEIGHT;
        }
    }

    if rect.min.x < monitor_rect.min.x {
        let shift = monitor_rect.min.x - rect.min.x;
        rect.min.x += shift;
        rect.max.x += shift;
    }
    if rect.max.x > monitor_rect.max.x {
        let shift = rect.max.x - monitor_rect.max.x;
        rect.min.x -= shift;
        rect.max.x -= shift;
    }
    if rect.min.y < monitor_rect.min.y {
        let shift = monitor_rect.min.y - rect.min.y;
        rect.min.y += shift;
        rect.max.y += shift;
    }
    if rect.max.y > monitor_rect.max.y {
        let shift = rect.max.y - monitor_rect.max.y;
        rect.min.y -= shift;
        rect.max.y -= shift;
    }

    rect
}

fn edge_from_monitor_position(
    pointer_position: egui::Pos2,
    monitor_rect: egui::Rect,
) -> Option<DockEdge> {
    let distances = [
        (
            DockEdge::Left,
            (pointer_position.x - monitor_rect.min.x).abs(),
        ),
        (
            DockEdge::Right,
            (monitor_rect.max.x - pointer_position.x).abs(),
        ),
        (
            DockEdge::Top,
            (pointer_position.y - monitor_rect.min.y).abs(),
        ),
        (
            DockEdge::Bottom,
            (monitor_rect.max.y - pointer_position.y).abs(),
        ),
    ];

    distances
        .into_iter()
        .filter(|(_, distance)| *distance <= SCREEN_EDGE_DROP_DISTANCE)
        .min_by(|(_, left), (_, right)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(edge, _)| edge)
}

fn clamp_pos_to_rect(position: egui::Pos2, rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(
        position.x.clamp(rect.min.x, rect.max.x),
        position.y.clamp(rect.min.y, rect.max.y),
    )
}

fn normal_window_position_for_cursor(
    cursor_position: egui::Pos2,
    pointer_offset: egui::Vec2,
    monitor_rect: egui::Rect,
    window_size: egui::Vec2,
) -> egui::Pos2 {
    clamp_window_position(cursor_position - pointer_offset, window_size, monitor_rect)
}

fn clamp_window_position(
    position: egui::Pos2,
    window_size: egui::Vec2,
    monitor_rect: egui::Rect,
) -> egui::Pos2 {
    let maximum_x = (monitor_rect.max.x - window_size.x).max(monitor_rect.min.x);
    let maximum_y = (monitor_rect.max.y - window_size.y).max(monitor_rect.min.y);

    egui::pos2(
        position.x.clamp(monitor_rect.min.x, maximum_x),
        position.y.clamp(monitor_rect.min.y, maximum_y),
    )
}

fn distance_to_rect(position: egui::Pos2, rect: egui::Rect) -> f32 {
    let dx = if position.x < rect.min.x {
        rect.min.x - position.x
    } else if position.x > rect.max.x {
        position.x - rect.max.x
    } else {
        0.0
    };
    let dy = if position.y < rect.min.y {
        rect.min.y - position.y
    } else if position.y > rect.max.y {
        position.y - rect.max.y
    } else {
        0.0
    };

    dx * dx + dy * dy
}

fn is_primary_mouse_button_down() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

        let state = unsafe { GetAsyncKeyState(VK_LBUTTON as i32) };
        return Some((state & i16::MIN) != 0);
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn get_global_cursor_position(pixels_per_point: f32) -> Option<egui::Pos2> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut point) };
        if ok != 0 {
            let scale = pixels_per_point.max(1.0);
            return Some(egui::pos2(point.x as f32 / scale, point.y as f32 / scale));
        }
    }

    let _ = pixels_per_point;
    None
}

fn get_docked_position(
    dock_edge: DockEdge,
    anchor_position: egui::Pos2,
    monitor_rect: egui::Rect,
    target_size: egui::Vec2,
) -> egui::Pos2 {
    let maximum_x = (monitor_rect.max.x - target_size.x).max(monitor_rect.min.x);
    let maximum_y = (monitor_rect.max.y - target_size.y).max(monitor_rect.min.y);
    let centered_x = anchor_position.x - target_size.x * 0.5;
    let centered_y = anchor_position.y - target_size.y * 0.5;

    match dock_edge {
        DockEdge::Left => egui::pos2(
            monitor_rect.min.x,
            centered_y.clamp(monitor_rect.min.y, maximum_y),
        ),
        DockEdge::Right => egui::pos2(maximum_x, centered_y.clamp(monitor_rect.min.y, maximum_y)),
        DockEdge::Top => egui::pos2(
            centered_x.clamp(monitor_rect.min.x, maximum_x),
            monitor_rect.min.y,
        ),
        DockEdge::Bottom => egui::pos2(centered_x.clamp(monitor_rect.min.x, maximum_x), maximum_y),
    }
}

fn format_action_button_label(item: &ActionItem) -> String {
    let name = shorten_text(item.name(), 30);
    match item {
        ActionItem::CopyText { .. } => format!("복사 · {name}"),
        ActionItem::RunApplication { .. } => format!("실행 · {name}"),
        ActionItem::OpenPath { .. } => format!("열기 · {name}"),
    }
}

fn show_copy_text_preview(ui: &mut egui::Ui, name: &str, text: &str) {
    ui.set_max_width(360.0);
    ui.label(egui::RichText::new(name).strong());
    ui.separator();

    let preview_text = if text.trim().is_empty() {
        "(내용 없음)"
    } else {
        text
    };

    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(preview_text).monospace()).wrap());
        });
}

fn show_editable_action_item(
    ui: &mut egui::Ui,
    index: usize,
    item: &mut EditableActionItem,
) -> Option<EditorActionRequest> {
    let mut request = None;

    ui.horizontal(|ui| {
        ui.label("종류");
        egui::ComboBox::from_id_salt(("action_kind", index))
            .selected_text(item.kind.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut item.kind, ActionKind::CopyText, "복사");
                ui.selectable_value(&mut item.kind, ActionKind::RunApplication, "실행");
                ui.selectable_value(&mut item.kind, ActionKind::OpenPath, "열기");
            });
    });

    ui.label("이름");
    ui.text_edit_singleline(&mut item.name);

    match item.kind {
        ActionKind::CopyText => {
            ui.label("복사할 내용");
            ui.add_sized(
                [ui.available_width(), 180.0],
                egui::TextEdit::multiline(&mut item.text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(9)
                    .lock_focus(true),
            );
            ui.label(
                egui::RichText::new(
                    "변수: {date} {time} {datetime} {clipboard} {selection} {input:라벨}",
                )
                .small()
                .weak(),
            );

            if ui.small_button("복사 테스트").clicked() {
                request = Some(EditorActionRequest::Test(item.to_action_item()));
            }
        }
        ActionKind::RunApplication => {
            ui.label("실행 파일 또는 명령");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut item.command)
                        .desired_width((ui.available_width() - 96.0).max(140.0)),
                );

                if ui.small_button("찾아보기...").clicked() {
                    match choose_executable_file() {
                        Ok(Some(path)) => {
                            item.command = path;
                            request = Some(EditorActionRequest::Status {
                                message: "실행 파일을 선택했습니다.".to_owned(),
                                is_error: false,
                            });
                        }
                        Ok(None) => {}
                        Err(error) => {
                            request = Some(EditorActionRequest::Status {
                                message: format!("파일 선택 실패: {error}"),
                                is_error: true,
                            });
                        }
                    }
                }
            });
            show_validation_message(ui, &check_application_command(&item.command));
            ui.label("인자");
            ui.text_edit_singleline(&mut item.arguments);

            if ui.small_button("테스트 실행").clicked() {
                request = Some(EditorActionRequest::Test(item.to_action_item()));
            }
        }
        ActionKind::OpenPath => {
            ui.label("파일/폴더 경로");
            ui.text_edit_singleline(&mut item.path);
            show_validation_message(ui, &check_open_path(&item.path));

            if ui.small_button("열기 테스트").clicked() {
                request = Some(EditorActionRequest::Test(item.to_action_item()));
            }
        }
    }

    request
}

fn show_validation_message(ui: &mut egui::Ui, message: &ValidationMessage) {
    let prefix = match message.severity {
        ValidationSeverity::Ok => "확인",
        ValidationSeverity::Warning => "주의",
        ValidationSeverity::Error => "문제",
    };

    ui.colored_label(
        validation_color(ui, message.severity),
        format!("{prefix}: {}", message.text),
    );
}

fn validation_color(ui: &egui::Ui, severity: ValidationSeverity) -> egui::Color32 {
    match severity {
        ValidationSeverity::Ok => egui::Color32::from_rgb(45, 150, 95),
        ValidationSeverity::Warning => egui::Color32::from_rgb(188, 128, 35),
        ValidationSeverity::Error => ui.visuals().error_fg_color,
    }
}

fn validate_editable_items_for_save(items: &[EditableActionItem]) -> Option<String> {
    for (index, item) in items.iter().enumerate() {
        let item_number = index + 1;
        let item_name = if item.name.trim().is_empty() {
            "(이름 없음)"
        } else {
            item.name.trim()
        };

        if item.name.trim().is_empty() {
            return Some(format!("{item_number}번째 항목: 이름이 비어 있습니다."));
        }

        let validation = match item.kind {
            ActionKind::CopyText => {
                if item.text.is_empty() {
                    Some(ValidationMessage::warning("복사할 내용이 비어 있습니다."))
                } else {
                    None
                }
            }
            ActionKind::RunApplication => Some(check_application_command(&item.command)),
            ActionKind::OpenPath => Some(check_open_path(&item.path)),
        };

        if let Some(validation) = validation {
            if validation.severity == ValidationSeverity::Error {
                return Some(format!(
                    "{item_number}번째 항목 '{item_name}' 저장 불가: {}",
                    validation.text
                ));
            }
        }
    }

    None
}

fn check_application_command(command: &str) -> ValidationMessage {
    let normalized_command = normalize_command_text(command);

    if normalized_command.is_empty() {
        return ValidationMessage::error("실행 파일 또는 명령이 비어 있습니다.");
    }

    if looks_like_path(&normalized_command) {
        let command_path = Path::new(&normalized_command);

        if command_path.is_file() {
            return ValidationMessage::ok(format!(
                "실행 파일을 확인했습니다: {}",
                command_path.display()
            ));
        }

        if command_path.is_dir() {
            return ValidationMessage::error(format!(
                "폴더입니다. 실행할 exe 파일을 선택하세요: {}",
                command_path.display()
            ));
        }

        return ValidationMessage::error(format!("파일이 없습니다: {}", command_path.display()));
    }

    if let Some(command_path) = find_application_command(&normalized_command) {
        return ValidationMessage::ok(format!(
            "PATH/Program Files에서 찾았습니다: {}",
            command_path.display()
        ));
    }

    ValidationMessage::error(format!(
        "PATH 또는 Program Files에서 찾을 수 없습니다: {normalized_command}"
    ))
}

fn check_open_path(path: &str) -> ValidationMessage {
    let normalized_path = normalize_command_text(path);

    if normalized_path.is_empty() {
        return ValidationMessage::error("파일/폴더 경로가 비어 있습니다.");
    }

    if looks_like_url(&normalized_path) {
        return ValidationMessage::warning(
            "웹 주소는 존재 여부를 확인하지 않습니다. 열기 테스트로 확인하세요.",
        );
    }

    let path = Path::new(&normalized_path);
    if path.is_dir() {
        return ValidationMessage::ok(format!("폴더를 확인했습니다: {}", path.display()));
    }

    if path.is_file() {
        return ValidationMessage::ok(format!("파일을 확인했습니다: {}", path.display()));
    }

    ValidationMessage::error(format!("파일/폴더가 없습니다: {}", path.display()))
}

fn resolve_application_command_for_spawn(command: &str) -> Result<String, String> {
    let normalized_command = normalize_command_text(command);

    if normalized_command.is_empty() {
        return Err("실행 파일 또는 명령이 비어 있습니다.".to_owned());
    }

    if looks_like_path(&normalized_command) {
        let command_path = Path::new(&normalized_command);

        if command_path.is_file() {
            return Ok(normalized_command);
        }

        if command_path.is_dir() {
            return Err(format!(
                "폴더라서 실행할 수 없습니다. exe 파일을 선택하세요: {}",
                command_path.display()
            ));
        }

        return Err(format!("파일이 없습니다: {}", command_path.display()));
    }

    if let Some(command_path) = find_application_command(&normalized_command) {
        return Ok(command_path.to_string_lossy().into_owned());
    }

    Err(format!(
        "PATH 또는 Program Files에서 찾을 수 없습니다: {normalized_command}"
    ))
}

fn resolve_open_path_for_spawn(path: &str) -> Result<String, String> {
    let normalized_path = normalize_command_text(path);

    if normalized_path.is_empty() {
        return Err("파일/폴더 경로가 비어 있습니다.".to_owned());
    }

    if looks_like_url(&normalized_path) || Path::new(&normalized_path).exists() {
        return Ok(normalized_path);
    }

    Err(format!(
        "파일/폴더가 없습니다: {}",
        Path::new(&normalized_path).display()
    ))
}

#[cfg(target_os = "windows")]
fn run_application_as_admin(command: &str, arguments: &[String]) -> Result<(), String> {
    use std::ffi::OsStr;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let resolved_command = resolve_application_command_for_spawn(command)?;
    let parameters = arguments
        .iter()
        .map(|argument| expand_environment_variables(argument))
        .map(|argument| {
            if argument.contains(char::is_whitespace) {
                format!("\"{argument}\"")
            } else {
                argument
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let verb = wide_null(OsStr::new("runas"));
    let file = wide_null(OsStr::new(&resolved_command));
    let parameters_wide = wide_null(OsStr::new(&parameters));

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            if parameters.is_empty() {
                std::ptr::null()
            } else {
                parameters_wide.as_ptr()
            },
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if (result as isize) <= 32 {
        Err(format!("ShellExecute 코드 {}", result as isize))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn run_application_as_admin(_command: &str, _arguments: &[String]) -> Result<(), String> {
    Err("관리자 권한 실행은 Windows에서만 지원합니다.".to_owned())
}

#[cfg(target_os = "windows")]
fn autostart_run_subkey() -> Vec<u16> {
    use std::ffi::OsStr;
    wide_null(OsStr::new(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    ))
}

#[cfg(target_os = "windows")]
fn is_autostart_enabled() -> bool {
    use std::ffi::OsStr;
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    let subkey = autostart_run_subkey();
    let value_name = wide_null(OsStr::new(AUTOSTART_VALUE_NAME));
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            &mut size,
        )
    };

    status == 0
}

#[cfg(not(target_os = "windows"))]
fn is_autostart_enabled() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn set_autostart(enabled: bool) -> Result<(), String> {
    use std::ffi::OsStr;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let subkey = autostart_run_subkey();
    let value_name = wide_null(OsStr::new(AUTOSTART_VALUE_NAME));

    let mut key_handle: HKEY = std::ptr::null_mut();
    let open_status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key_handle,
            std::ptr::null_mut(),
        )
    };
    if open_status != 0 {
        return Err(format!("레지스트리 열기 실패 (코드 {open_status})"));
    }

    let result = if enabled {
        let executable = env::current_exe().map_err(|error| error.to_string())?;
        let command = format!("\"{}\"", executable.display());
        let data = wide_null(OsStr::new(&command));
        let byte_length = (data.len() * 2) as u32;
        let status = unsafe {
            RegSetValueExW(
                key_handle,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                byte_length,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!("등록 실패 (코드 {status})"))
        }
    } else {
        let status = unsafe { RegDeleteValueW(key_handle, value_name.as_ptr()) };
        if status == 0 || status == 2 {
            Ok(())
        } else {
            Err(format!("해제 실패 (코드 {status})"))
        }
    };

    unsafe {
        RegCloseKey(key_handle);
    }

    result
}

#[cfg(not(target_os = "windows"))]
fn set_autostart(_enabled: bool) -> Result<(), String> {
    Err("자동 실행은 Windows에서만 지원합니다.".to_owned())
}

#[cfg(target_os = "windows")]
fn build_tray_state(autostart_enabled: bool) -> Result<TrayState, String> {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    let open_item = MenuItem::with_id("tray_open", "열기", true, None);
    let settings_item = MenuItem::with_id("tray_settings", "설정", true, None);
    let autostart_item = CheckMenuItem::with_id(
        "tray_autostart",
        "Windows 시작 시 실행",
        true,
        autostart_enabled,
        None,
    );
    let quit_item = MenuItem::with_id("tray_quit", "종료", true, None);

    menu.append(&open_item).map_err(|error| error.to_string())?;
    menu.append(&settings_item)
        .map_err(|error| error.to_string())?;
    menu.append(&autostart_item)
        .map_err(|error| error.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| error.to_string())?;
    menu.append(&quit_item).map_err(|error| error.to_string())?;

    let icon = build_tray_icon_image()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Quick Dock")
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| error.to_string())?;

    Ok(TrayState {
        _icon: tray,
        autostart_item,
    })
}

#[cfg(target_os = "windows")]
fn build_tray_icon_image() -> Result<tray_icon::Icon, String> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let index = ((y * SIZE + x) * 4) as usize;
            let is_border = x < 2 || y < 2 || x >= SIZE - 2 || y >= SIZE - 2;
            let (r, g, b, a) = if is_border {
                (0, 0, 0, 0)
            } else {
                (18, 140, 126, 255)
            };
            rgba[index] = r;
            rgba[index + 1] = g;
            rgba[index + 2] = b;
            rgba[index + 3] = a;
        }
    }

    for y in 8..24 {
        for x in 9..12 {
            let index = ((y * SIZE + x) * 4) as usize;
            rgba[index] = 235;
            rgba[index + 1] = 245;
            rgba[index + 2] = 245;
            rgba[index + 3] = 255;
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).map_err(|error| error.to_string())
}

fn reveal_in_file_explorer(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let target = Path::new(path);
        if !target.exists() {
            return Err(format!("경로가 없습니다: {}", target.display()));
        }

        Command::new("explorer.exe")
            .arg(format!("/select,{path}"))
            .spawn()
            .map(|_| ())
            .map_err(|error| describe_spawn_error(&error, path))
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = Path::new(path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(path));
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn normalize_command_text(text: &str) -> String {
    let expanded_text = expand_environment_variables(text);
    trim_wrapping_quotes(expanded_text.trim()).trim().to_owned()
}

fn looks_like_url(text: &str) -> bool {
    let lower_text = text.to_ascii_lowercase();
    lower_text.starts_with("http://")
        || lower_text.starts_with("https://")
        || lower_text.starts_with("mailto:")
}

fn describe_spawn_error(error: &std::io::Error, target: &str) -> String {
    if let Some(error_code) = error.raw_os_error() {
        match error_code {
            2 | 3 => return format!("파일 없음: {target}"),
            5 => return format!("권한 문제: 접근이 거부되었습니다. ({target})"),
            193 => return format!("파일 형식 문제: 실행 가능한 Windows 앱이 아닙니다. ({target})"),
            740 => return format!("권한 문제: 관리자 권한이 필요할 수 있습니다. ({target})"),
            _ => {}
        }
    }

    match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("파일 또는 PATH 항목을 찾지 못했습니다. ({target})")
        }
        std::io::ErrorKind::PermissionDenied => {
            format!("권한 문제로 실행하지 못했습니다. ({target})")
        }
        std::io::ErrorKind::InvalidInput => format!("명령 또는 인자 형식 문제입니다. ({target})"),
        _ => format!("{error} ({target})"),
    }
}

#[cfg(target_os = "windows")]
fn choose_executable_file() -> std::io::Result<Option<String>> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = '실행 파일 선택'
$dialog.Filter = '실행 파일 (*.exe)|*.exe|모든 파일 (*.*)|*.*'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dialog.FileName
}
"#;

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-Command", script])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            if stderr.is_empty() {
                format!("PowerShell 종료 코드: {}", output.status)
            } else {
                stderr
            },
        ));
    }

    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if selected_path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selected_path))
    }
}

#[cfg(not(target_os = "windows"))]
fn choose_executable_file() -> std::io::Result<Option<String>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "파일 선택은 Windows에서만 지원합니다.",
    ))
}

fn run_explorer_cleanup() -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
function Normalize-ExplorerPath([string]$path) {
    try {
        return [System.IO.Path]::GetFullPath($path).TrimEnd('\')
    } catch {
        return $path.TrimEnd('\')
    }
}

function Test-IsChildPath([string]$parent, [string]$child) {
    if ($parent.Length -eq 0 -or $child.Length -le $parent.Length) { return $false }
    return $child.StartsWith($parent + '\', [System.StringComparison]::OrdinalIgnoreCase)
}

$shell = New-Object -ComObject Shell.Application
$entries = New-Object System.Collections.Generic.List[object]
$index = 0

foreach ($window in @($shell.Windows())) {
    try {
        if (-not $window.FullName.ToLowerInvariant().EndsWith('explorer.exe')) { continue }
        $path = $window.Document.Folder.Self.Path
        if ([string]::IsNullOrWhiteSpace($path)) { continue }

        $entries.Add([pscustomobject]@{
            Window = $window
            Path = Normalize-ExplorerPath $path
            Index = $index
        })
        $index++
    } catch {}
}

$keepIndexes = New-Object System.Collections.Generic.HashSet[int]
$paths = @($entries | Select-Object -ExpandProperty Path -Unique)

foreach ($path in $paths) {
    $first = @($entries | Where-Object { $_.Path -ieq $path } | Sort-Object Index | Select-Object -First 1)
    if ($first.Count -gt 0) { [void]$keepIndexes.Add($first[0].Index) }
}

foreach ($entry in $entries) {
    $hasOpenParent = $false
    foreach ($path in $paths) {
        if (Test-IsChildPath $path $entry.Path) {
            $hasOpenParent = $true
            break
        }
    }

    if (-not $keepIndexes.Contains($entry.Index) -or $hasOpenParent) {
        try { $entry.Window.Quit() } catch {}
    }
}
"#;

        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .status()?;

        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("PowerShell 종료 코드: {status}"),
            ));
        }
    }

    Ok(())
}

fn shorten_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let shortened: String = chars.by_ref().take(max_chars).collect();

    if chars.next().is_some() {
        format!("{shortened}...")
    } else {
        shortened
    }
}

fn configure_system_fonts(context: &egui::Context) {
    if let Some(font_bytes) = load_first_existing_font(system_font_candidates()) {
        let mut fonts = egui::FontDefinitions::default();
        let font_name = "quick_dock_system_cjk".to_owned();

        fonts.font_data.insert(
            font_name.clone(),
            Arc::new(egui::FontData::from_owned(font_bytes)),
        );

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, font_name.clone());
        }

        context.set_fonts(fonts);
    }
}

fn load_first_existing_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| fs::read(path).ok())
}

fn system_font_candidates() -> &'static [&'static str] {
    &[
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\malgunbd.ttf",
        r"C:\Windows\Fonts\gulim.ttc",
        r"C:\Windows\Fonts\batang.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ]
}

fn get_configuration_path() -> PathBuf {
    configuration_directory().join(CONFIGURATION_FILE_NAME)
}

fn get_log_path() -> PathBuf {
    configuration_directory().join(LOG_FILE_NAME)
}

fn configuration_directory() -> PathBuf {
    let executable_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let executable_directory = executable_path.parent().unwrap_or_else(|| Path::new("."));
    executable_directory.join(CONFIGURATION_DIRECTORY_NAME)
}

fn log_event(message: &str) {
    use std::io::Write;

    let log_path = get_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let (year, month, day, hour, minute, second) = current_local_datetime();
    let line =
        format!("[{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}] {message}\n");

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn load_tabs_from_configuration(
    configuration_path: &Path,
) -> (Vec<QuickDockTab>, LayoutSettings, String) {
    let was_created = ensure_configuration_file(configuration_path);

    match fs::read_to_string(configuration_path) {
        Ok(configuration_text) => {
            let layout = parse_layout_from_ini(&configuration_text);

            match parse_tabs_from_ini(&configuration_text) {
                Ok(tabs) => {
                    let item_count: usize = tabs.iter().map(|tab| tab.items.len()).sum();
                    let tab_count = tabs.len();
                    let message = if was_created {
                        format!("기본 설정 생성 완료: {tab_count}개 탭, {item_count}개 항목")
                    } else {
                        format!("설정 읽기 완료: {tab_count}개 탭, {item_count}개 항목")
                    };
                    (tabs, layout, message)
                }
                Err(error) => {
                    log_event(&format!("설정 파싱 실패: {error}"));
                    (
                        vec![QuickDockTab::new("기본".to_owned(), get_default_items())],
                        layout,
                        format!("설정 파싱 실패. 기본 항목 사용: {error}"),
                    )
                }
            }
        }
        Err(error) => {
            log_event(&format!("설정 파일 읽기 실패: {error}"));
            (
                vec![QuickDockTab::new("기본".to_owned(), get_default_items())],
                LayoutSettings::default(),
                format!("설정 파일 읽기 실패. 기본 항목 사용: {error}"),
            )
        }
    }
}

fn ensure_configuration_file(configuration_path: &Path) -> bool {
    if configuration_path.exists() {
        return false;
    }

    if let Some(parent) = configuration_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::write(configuration_path, get_default_ini_text()).is_ok()
}

fn save_tabs_to_configuration(
    configuration_path: &Path,
    tabs: &[QuickDockTab],
    layout: &LayoutSettings,
    backup: bool,
) -> std::io::Result<()> {
    if let Some(parent) = configuration_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if backup {
        backup_configuration_file(configuration_path)?;
    }

    fs::write(configuration_path, serialize_tabs_to_ini(tabs, layout))
}

fn backup_configuration_file(configuration_path: &Path) -> std::io::Result<()> {
    if !configuration_path.exists() {
        return Ok(());
    }

    let Some(file_name) = configuration_path.file_name() else {
        return Ok(());
    };

    let backup_file_name = format!("{}.bak", file_name.to_string_lossy());
    let backup_path = configuration_path.with_file_name(backup_file_name);
    fs::copy(configuration_path, backup_path)?;

    Ok(())
}

fn serialize_tabs_to_ini(tabs: &[QuickDockTab], layout: &LayoutSettings) -> String {
    let expanded_size = clamp_expanded_size(layout.expanded_size);
    let window_size = clamp_expanded_size(layout.window_size);
    let mut output = String::from(
        "; Quick Dock 설정 파일\n; 앱 안의 설정 화면에서 편집할 수 있습니다.\n; 실행 인자는 | 로 구분합니다.\n\n",
    );

    output.push_str("[layout]\n");
    output.push_str(&format!("schema_version={SCHEMA_VERSION}\n"));
    output.push_str(&format!("expanded_width={:.0}\n", expanded_size.x));
    output.push_str(&format!("expanded_height={:.0}\n", expanded_size.y));
    output.push_str(&format!("window_width={:.0}\n", window_size.x));
    output.push_str(&format!("window_height={:.0}\n", window_size.y));
    output.push_str(&format!("dock_edge={}\n\n", layout.dock_edge.ini_value()));

    for (monitor_key, state) in layout.monitors.iter() {
        output.push_str(&format!("[monitor.{}]\n", escape_ini_value(monitor_key)));
        output.push_str(&format!("edge={}\n", state.edge.ini_value()));
        output.push_str(&format!("anchor_x={:.0}\n", state.anchor.x));
        output.push_str(&format!("anchor_y={:.0}\n", state.anchor.y));
        output.push_str(&format!("expanded_width={:.0}\n", state.expanded_size.x));
        output.push_str(&format!("expanded_height={:.0}\n\n", state.expanded_size.y));
    }

    for (tab_index, tab) in tabs.iter().enumerate() {
        output.push_str(&format!("[tab.{}]\n", tab_index + 1));
        output.push_str(&format!("name={}\n\n", escape_ini_value(&tab.name)));

        for (item_index, item) in tab.items.iter().enumerate() {
            let editable_item = EditableActionItem::from_action_item(item);

            output.push_str(&format!(
                "[tab.{}.item.{}]\n",
                tab_index + 1,
                item_index + 1
            ));
            output.push_str(&format!("kind={}\n", editable_item.kind.ini_value()));
            output.push_str(&format!("name={}\n", escape_ini_value(&editable_item.name)));

            match editable_item.kind {
                ActionKind::CopyText => {
                    output.push_str(&format!("text={}\n", escape_ini_value(&editable_item.text)));
                }
                ActionKind::RunApplication => {
                    output.push_str(&format!(
                        "command={}\n",
                        escape_ini_value(&editable_item.command)
                    ));
                    output.push_str(&format!(
                        "arguments={}\n",
                        escape_ini_value(&editable_item.arguments)
                    ));
                }
                ActionKind::OpenPath => {
                    output.push_str(&format!("path={}\n", escape_ini_value(&editable_item.path)));
                }
            }

            output.push('\n');
        }
    }

    output
}

fn escape_ini_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn parse_layout_from_ini(configuration_text: &str) -> LayoutSettings {
    let Ok(sections) = parse_ini_sections(configuration_text) else {
        return LayoutSettings::default();
    };

    let mut layout = LayoutSettings::default();

    if let Some((_, properties)) = sections
        .iter()
        .find(|(section_name, _)| section_name.eq_ignore_ascii_case("layout"))
    {
        if let Some(width) = parse_f32_value(properties, "expanded_width") {
            layout.expanded_size.x = width;
        }
        if let Some(height) = parse_f32_value(properties, "expanded_height") {
            layout.expanded_size.y = height;
        }
        layout.expanded_size = clamp_expanded_size(layout.expanded_size);

        if let Some(width) = parse_f32_value(properties, "window_width") {
            layout.window_size.x = width;
        }
        if let Some(height) = parse_f32_value(properties, "window_height") {
            layout.window_size.y = height;
        }
        layout.window_size = clamp_expanded_size(layout.window_size);

        if let Some(edge) =
            optional_ini_value(properties, "dock_edge").and_then(|value| DockEdge::from_ini_value(&value))
        {
            layout.dock_edge = edge;
        }
    }

    for (section_name, properties) in sections.iter() {
        if section_name.len() <= 8 || !section_name[..8].eq_ignore_ascii_case("monitor.") {
            continue;
        }

        let monitor_key = section_name[8..].to_owned();
        if monitor_key.is_empty() {
            continue;
        }

        let edge = optional_ini_value(properties, "edge")
            .and_then(|value| DockEdge::from_ini_value(&value))
            .unwrap_or(layout.dock_edge);
        let anchor = egui::pos2(
            parse_f32_value(properties, "anchor_x").unwrap_or(0.0),
            parse_f32_value(properties, "anchor_y").unwrap_or(0.0),
        );
        let expanded_size = clamp_expanded_size(egui::vec2(
            parse_f32_value(properties, "expanded_width").unwrap_or(layout.expanded_size.x),
            parse_f32_value(properties, "expanded_height").unwrap_or(layout.expanded_size.y),
        ));

        layout.monitors.insert(
            monitor_key,
            MonitorDockState {
                edge,
                anchor,
                expanded_size,
            },
        );
    }

    layout
}

fn parse_f32_value(properties: &BTreeMap<String, String>, key: &str) -> Option<f32> {
    optional_ini_value(properties, key).and_then(|value| value.parse::<f32>().ok())
}

fn monitor_key(rect: egui::Rect) -> String {
    format!(
        "{}x{}+{}+{}",
        rect.width().round() as i32,
        rect.height().round() as i32,
        rect.min.x.round() as i32,
        rect.min.y.round() as i32,
    )
}

fn parse_tabs_from_ini(configuration_text: &str) -> Result<Vec<QuickDockTab>, String> {
    let sections = parse_ini_sections(configuration_text)?;
    let has_tab_sections = sections
        .iter()
        .any(|(section_name, _)| section_name.to_ascii_lowercase().starts_with("tab."));

    if !has_tab_sections {
        return Ok(vec![QuickDockTab::new(
            "기본".to_owned(),
            parse_items_from_sections(&sections)?,
        )]);
    }

    let mut tab_names = BTreeMap::<usize, String>::new();
    let mut tab_items = BTreeMap::<usize, Vec<ActionItem>>::new();

    for (section_name, properties) in sections {
        let lower_section_name = section_name.to_ascii_lowercase();
        if !lower_section_name.starts_with("tab.") {
            continue;
        }

        let section_parts: Vec<&str> = lower_section_name.split('.').collect();
        let Some(tab_number) = section_parts
            .get(1)
            .and_then(|value| value.parse::<usize>().ok())
        else {
            continue;
        };

        if section_parts.len() == 2 {
            let name = optional_ini_value(&properties, "name")
                .unwrap_or_else(|| format!("탭 {tab_number}"));
            tab_names.insert(tab_number, name);
        } else if section_parts.len() >= 4 && section_parts[2] == "item" {
            tab_items
                .entry(tab_number)
                .or_default()
                .push(parse_action_item_from_properties(
                    &section_name,
                    &properties,
                )?);
        }
    }

    let mut tab_numbers: Vec<usize> = tab_names.keys().chain(tab_items.keys()).copied().collect();
    tab_numbers.sort_unstable();
    tab_numbers.dedup();

    let mut tabs = Vec::new();
    for tab_number in tab_numbers {
        let name = tab_names
            .remove(&tab_number)
            .unwrap_or_else(|| format!("탭 {tab_number}"));
        let items = tab_items.remove(&tab_number).unwrap_or_default();
        tabs.push(QuickDockTab::new(name, items));
    }

    if tabs.is_empty() {
        tabs.push(QuickDockTab::new("기본".to_owned(), Vec::new()));
    }

    Ok(tabs)
}

fn parse_ini_sections(
    configuration_text: &str,
) -> Result<Vec<(String, BTreeMap<String, String>)>, String> {
    let mut sections: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    let mut current_section_name: Option<String> = None;
    let mut current_properties = BTreeMap::new();

    for (line_index, raw_line) in configuration_text.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if let Some(section_name) = current_section_name.replace(
                line.trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_owned(),
            ) {
                sections.push((section_name, std::mem::take(&mut current_properties)));
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("{}번째 줄에 '='가 없습니다.", line_index + 1));
        };

        if current_section_name.is_none() {
            return Err(format!("{}번째 줄이 섹션 밖에 있습니다.", line_index + 1));
        }

        current_properties.insert(
            key.trim().to_ascii_lowercase(),
            normalize_ini_value(value.trim()),
        );
    }

    if let Some(section_name) = current_section_name {
        sections.push((section_name, current_properties));
    }

    Ok(sections)
}

fn parse_items_from_sections(
    sections: &[(String, BTreeMap<String, String>)],
) -> Result<Vec<ActionItem>, String> {
    let mut items = Vec::new();
    for (section_name, properties) in sections.iter() {
        if !section_name.to_ascii_lowercase().starts_with("item") {
            continue;
        }

        items.push(parse_action_item_from_properties(section_name, properties)?);
    }

    Ok(items)
}

fn parse_action_item_from_properties(
    section_name: &str,
    properties: &BTreeMap<String, String>,
) -> Result<ActionItem, String> {
    let kind = required_ini_value(properties, "kind", section_name)?;
    let name = required_ini_value(properties, "name", section_name)?;

    match kind.as_str() {
        "copy_text" => Ok(ActionItem::CopyText {
            name,
            text: required_ini_value(properties, "text", section_name)?,
        }),
        "run_app" => Ok(ActionItem::RunApplication {
            name,
            command: required_ini_value(properties, "command", section_name)?,
            arguments: optional_ini_value(properties, "arguments")
                .map(|value| parse_argument_list(&value))
                .unwrap_or_default(),
        }),
        "open_path" => Ok(ActionItem::OpenPath {
            name,
            path: required_ini_value(properties, "path", section_name)?,
        }),
        _ => Err(format!(
            "[{section_name}]의 kind '{kind}'를 알 수 없습니다."
        )),
    }
}

fn required_ini_value(
    properties: &BTreeMap<String, String>,
    key: &str,
    section_name: &str,
) -> Result<String, String> {
    properties
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("[{section_name}]에 {key} 값이 없습니다."))
}

fn optional_ini_value(properties: &BTreeMap<String, String>, key: &str) -> Option<String> {
    properties
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
}

fn parse_argument_list(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_ini_value(value: &str) -> String {
    let trimmed_value = value.trim();
    let unquoted_value = if trimmed_value.len() >= 2
        && trimmed_value.starts_with('"')
        && trimmed_value.ends_with('"')
    {
        &trimmed_value[1..trimmed_value.len() - 1]
    } else {
        trimmed_value
    };

    unescape_ini_value(unquoted_value)
}

fn unescape_ini_value(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => result.push('\n'),
            Some('r') => result.push('\r'),
            Some('t') => result.push('\t'),
            Some('\\') => result.push('\\'),
            Some('"') => result.push('"'),
            Some(other) => {
                result.push('\\');
                result.push(other);
            }
            None => result.push('\\'),
        }
    }

    result
}

fn get_default_ini_text() -> &'static str {
    r#"; Quick Dock 설정 파일
; 앱 안의 설정 화면에서 편집할 수 있습니다.
; 줄바꿈은 \n 으로 입력합니다.
; 실행 인자는 arguments=a|b|c 처럼 | 로 구분합니다.

[layout]
schema_version=1
expanded_width=350
expanded_height=430

[tab.1]
name=기본

[tab.1.item.1]
kind=copy_text
name=Jira - 검토 완료
text=검토 완료했습니다.\n\n확인 내용:\n- \n\n조치 사항:\n- 없음\n\n특이 사항:\n- 없음\n

[tab.1.item.2]
kind=copy_text
name=Jira - 상세 설명 요청
text=자세히 설명해 주세요.\n\n확인이 필요한 내용:\n- 재현 절차:\n- 기대 결과:\n- 실제 결과:\n- 관련 로그 또는 화면:\n

[tab.1.item.3]
kind=copy_text
name=Jira - 조치 기록
text=조치 내용:\n- \n\n원인:\n- \n\n변경 사항:\n- \n\n확인 결과:\n- 정상 확인\n

[tab.1.item.4]
kind=copy_text
name=작업 완료 (티켓 입력)
text=작업 완료했습니다. 티켓: {input:티켓 번호} ({datetime})

[tab.1.item.5]
kind=run_app
name=메모장 실행
command=notepad.exe
arguments=

[tab.1.item.6]
kind=open_path
name=다운로드 폴더 열기
path=C:\Users\%USERNAME%\Downloads
"#
}

fn get_default_items() -> Vec<ActionItem> {
    vec![
        ActionItem::CopyText {
            name: "Jira - 검토 완료".to_owned(),
            text: "검토 완료했습니다.\n\n확인 내용:\n- \n\n조치 사항:\n- 없음\n\n특이 사항:\n- 없음\n".to_owned(),
        },
        ActionItem::CopyText {
            name: "Jira - 상세 설명 요청".to_owned(),
            text: "자세히 설명해 주세요.\n\n확인이 필요한 내용:\n- 재현 절차:\n- 기대 결과:\n- 실제 결과:\n- 관련 로그 또는 화면:\n".to_owned(),
        },
        ActionItem::RunApplication {
            name: "메모장 실행".to_owned(),
            command: "notepad.exe".to_owned(),
            arguments: Vec::new(),
        },
    ]
}

#[cfg(target_os = "windows")]
fn find_application_command(command: &str) -> Option<PathBuf> {
    find_windows_application_command(command)
}

#[cfg(not(target_os = "windows"))]
fn find_application_command(command: &str) -> Option<PathBuf> {
    let path_value = env::var_os("PATH")?;

    for directory in env::split_paths(&path_value) {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn trim_wrapping_quotes(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

fn looks_like_path(command: &str) -> bool {
    command.contains('\\') || command.contains('/') || command.contains(':')
}

#[cfg(target_os = "windows")]
fn find_windows_application_command(command: &str) -> Option<PathBuf> {
    let command_names = windows_command_names(command);

    if let Some(path) = find_command_in_path(&command_names) {
        return Some(path);
    }

    for directory in windows_application_search_directories(command) {
        for command_name in &command_names {
            let candidate = directory.join(command_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_command_names(command: &str) -> Vec<String> {
    let path = Path::new(command);

    if path.extension().is_some() {
        vec![command.to_owned()]
    } else {
        vec![command.to_owned(), format!("{command}.exe")]
    }
}

#[cfg(target_os = "windows")]
fn find_command_in_path(command_names: &[String]) -> Option<PathBuf> {
    let path_value = env::var_os("PATH")?;

    for directory in env::split_paths(&path_value) {
        for command_name in command_names {
            let candidate = directory.join(command_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_application_search_directories(command: &str) -> Vec<PathBuf> {
    let normalized_command = command.trim_end_matches(".exe");
    let folder_names = known_windows_application_folder_names(normalized_command);
    let roots = [
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
        env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Programs").into()),
    ];
    let mut directories = Vec::new();

    for root in roots.into_iter().flatten() {
        let root_path = PathBuf::from(root);
        for folder_name in &folder_names {
            directories.push(root_path.join(folder_name));
        }
    }

    directories
}

#[cfg(target_os = "windows")]
fn known_windows_application_folder_names(command_stem: &str) -> Vec<String> {
    let mut folder_names = vec![command_stem.to_owned()];

    if command_stem.eq_ignore_ascii_case("notepad++") {
        folder_names.push("Notepad++".to_owned());
    }

    folder_names
}

fn extract_input_labels(template: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let mut rest = template;

    while let Some(open_index) = rest.find('{') {
        let after_open = &rest[open_index + 1..];
        let Some(close_index) = after_open.find('}') else {
            break;
        };

        let token = &after_open[..close_index];
        if let Some(label) = token.strip_prefix("input:") {
            let label = label.trim().to_owned();
            if !label.is_empty() && !labels.contains(&label) {
                labels.push(label);
            }
        }

        rest = &after_open[close_index + 1..];
    }

    labels
}

fn expand_copy_template(
    template: &str,
    inputs: &[TemplateInputField],
    selection_text: &str,
) -> String {
    let clipboard_text = if template.contains("{clipboard}") {
        read_clipboard_text().unwrap_or_default()
    } else {
        String::new()
    };

    let (year, month, day, hour, minute, second) = current_local_datetime();
    let date_text = format!("{year:04}-{month:02}-{day:02}");
    let time_text = format!("{hour:02}:{minute:02}:{second:02}");
    let datetime_text = format!("{date_text} {time_text}");

    let mut result = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open_index) = rest.find('{') {
        result.push_str(&rest[..open_index]);
        let after_open = &rest[open_index + 1..];

        let Some(close_index) = after_open.find('}') else {
            result.push('{');
            rest = after_open;
            continue;
        };

        let token = &after_open[..close_index];
        let replacement = match token {
            "date" => Some(date_text.clone()),
            "time" => Some(time_text.clone()),
            "datetime" => Some(datetime_text.clone()),
            "clipboard" => Some(clipboard_text.clone()),
            "selection" => Some(selection_text.to_owned()),
            other => other.strip_prefix("input:").map(|label| {
                let label = label.trim();
                inputs
                    .iter()
                    .find(|field| field.label == label)
                    .map(|field| field.value.clone())
                    .unwrap_or_default()
            }),
        };

        match replacement {
            Some(value) => result.push_str(&value),
            None => {
                result.push('{');
                result.push_str(token);
                result.push('}');
            }
        }

        rest = &after_open[close_index + 1..];
    }

    result.push_str(rest);
    result
}

fn read_clipboard_text() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}

fn current_local_datetime() -> (i32, u32, u32, u32, u32, u32) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;

        let mut system_time: windows_sys::Win32::Foundation::SYSTEMTIME =
            unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut system_time) };

        return (
            system_time.wYear as i32,
            system_time.wMonth as u32,
            system_time.wDay as u32,
            system_time.wHour as u32,
            system_time.wMinute as u32,
            system_time.wSecond as u32,
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        let total_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        civil_datetime_from_unix(total_seconds)
    }
}

#[cfg(not(target_os = "windows"))]
fn civil_datetime_from_unix(total_seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let hour = (seconds_of_day / 3600) as u32;
    let minute = ((seconds_of_day % 3600) / 60) as u32;
    let second = (seconds_of_day % 60) as u32;

    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = (if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    }) as u32;
    let year = (year + if month <= 2 { 1 } else { 0 }) as i32;

    (year, month, day, hour, minute, second)
}

fn current_external_foreground_window() -> Option<isize> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

        unsafe {
            let handle = GetForegroundWindow();
            if handle.is_null() {
                return None;
            }

            let mut buffer = [0u16; 256];
            let length = GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32);
            if length > 0 {
                let title = String::from_utf16_lossy(&buffer[..length as usize]);
                if title == "Quick Dock" {
                    return None;
                }
            }

            Some(handle as isize)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn capture_selection_text(previous_window: Option<isize>) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

        let handle = previous_window? as *mut core::ffi::c_void;
        unsafe {
            SetForegroundWindow(handle);
            send_copy_command();
        }
        std::thread::sleep(Duration::from_millis(140));
        read_clipboard_text().filter(|text| !text.is_empty())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = previous_window;
        None
    }
}

#[cfg(target_os = "windows")]
unsafe fn send_copy_command() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VK_C: u16 = 0x43;

    fn key_event(virtual_key: u16, key_up: bool) -> INPUT {
        let mut input: INPUT = unsafe { std::mem::zeroed() };
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki = KEYBDINPUT {
            wVk: virtual_key,
            wScan: 0,
            dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            dwExtraInfo: 0,
        };
        input
    }

    let inputs = [
        key_event(VK_CONTROL, false),
        key_event(VK_C, false),
        key_event(VK_C, true),
        key_event(VK_CONTROL, true),
    ];

    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );
}

fn expand_environment_variables(text: &str) -> String {
    let mut expanded_text = String::new();
    let mut remaining_text = text;

    while let Some(start_index) = remaining_text.find('%') {
        expanded_text.push_str(&remaining_text[..start_index]);
        remaining_text = &remaining_text[start_index + 1..];

        if let Some(end_index) = remaining_text.find('%') {
            let variable_name = &remaining_text[..end_index];
            if let Ok(variable_value) = env::var(variable_name) {
                expanded_text.push_str(&variable_value);
            } else {
                expanded_text.push('%');
                expanded_text.push_str(variable_name);
                expanded_text.push('%');
            }
            remaining_text = &remaining_text[end_index + 1..];
        } else {
            expanded_text.push('%');
            expanded_text.push_str(remaining_text);
            return expanded_text;
        }
    }

    expanded_text.push_str(remaining_text);
    expanded_text
}
