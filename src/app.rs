use std::collections::BTreeMap;
#[cfg(target_os = "windows")]
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::sync::Mutex;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use eframe::egui;

use crate::constants::*;
use crate::model::*;
use crate::layout::*;
use crate::widgets::*;
use crate::config::*;
use crate::commands::*;
use crate::platform::*;
#[cfg(target_os = "windows")]
use crate::tray::{build_tray_state, TrayCommand, TrayState};

pub(crate) struct QuickDockApplication {
    pub(crate) configuration_path: PathBuf,
    pub(crate) tabs: Vec<QuickDockTab>,
    pub(crate) active_tab_index: usize,
    pub(crate) next_tab_number: usize,
    pub(crate) new_item_kind: ActionKind,
    pub(crate) dock_edge: DockEdge,
    pub(crate) highlighted_dock_edge: Option<DockEdge>,
    pub(crate) drag_drop_target: Option<DragDropTarget>,
    pub(crate) drag_pointer_offset: egui::Vec2,
    pub(crate) expanded_resize_drag: Option<ResizeDrag>,
    pub(crate) window_resize_drag: Option<ResizeDrag>,
    pub(crate) expanded_size: egui::Vec2,
    pub(crate) window_size: egui::Vec2,
    pub(crate) monitor_dock_states: BTreeMap<String, MonitorDockState>,
    pub(crate) renaming_tab_index: Option<usize>,
    pub(crate) renaming_focus_pending: bool,
    pub(crate) is_settings_editor_open: bool,
    pub(crate) is_docked: bool,
    pub(crate) is_expanded: bool,
    pub(crate) is_dragging: bool,
    pub(crate) last_hovered_at: Instant,
    pub(crate) last_status_message: String,
    pub(crate) toast_message: Option<String>,
    pub(crate) toast_started_at: Instant,
    pub(crate) toast_is_error: bool,
    pub(crate) last_known_position: egui::Pos2,
    pub(crate) last_known_inner_position: egui::Pos2,
    pub(crate) last_known_monitor_rect: egui::Rect,
    pub(crate) available_monitor_rects: Vec<egui::Rect>,
    pub(crate) dock_anchor_position: egui::Pos2,
    pub(crate) pending_input_copy: Option<PendingInputCopy>,
    pub(crate) last_external_foreground_window: Option<isize>,
    pub(crate) editor_focus_index: Option<usize>,
    pub(crate) palette_open: bool,
    pub(crate) palette_query: String,
    pub(crate) palette_selected: usize,
    pub(crate) palette_focus_pending: bool,
    pub(crate) recent_items: Vec<ActionItem>,
    pub(crate) autostart_enabled: bool,
    #[cfg(target_os = "windows")]
    pub(crate) tray_state: Option<TrayState>,
    #[cfg(target_os = "windows")]
    pub(crate) tray_attempted: bool,
    #[cfg(target_os = "windows")]
    pub(crate) tray_commands: Arc<Mutex<VecDeque<TrayCommand>>>,
}

pub(crate) struct PaletteEntry {
    pub(crate) item: ActionItem,
    pub(crate) tab_index: Option<usize>,
    pub(crate) tab_label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingInputCopy {
    pub(crate) name: String,
    pub(crate) template: String,
    pub(crate) fields: Vec<TemplateInputField>,
    pub(crate) focus_pending: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateInputField {
    pub(crate) label: String,
    pub(crate) value: String,
}

impl QuickDockApplication {
    pub(crate) fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
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
            #[cfg(target_os = "windows")]
            tray_commands: Arc::new(Mutex::new(VecDeque::new())),
        }
    }


    pub(crate) fn reload_configuration(&mut self) {
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


    pub(crate) fn update_window_state(&mut self, context: &egui::Context, frame: &eframe::Frame) {
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


    pub(crate) fn update_monitor_rect(
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


    pub(crate) fn begin_title_bar_drag(&mut self, context: &egui::Context) {
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


    pub(crate) fn update_drag_preview(&mut self, context: &egui::Context) {
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


    pub(crate) fn finish_title_bar_drag(&mut self, context: &egui::Context) {
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


    pub(crate) fn finish_window_mode_at(
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


    pub(crate) fn send_drag_preview_geometry(
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


    pub(crate) fn current_cursor_position(&self, context: &egui::Context) -> Option<egui::Pos2> {
        get_global_cursor_position(context.pixels_per_point()).or_else(|| {
            context
                .input(|input| input.pointer.interact_pos())
                .map(|position| self.pointer_to_monitor_position(position))
        })
    }


    pub(crate) fn monitor_rect_for_position(&self, position: egui::Pos2) -> egui::Rect {
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


    pub(crate) fn begin_expanded_resize(&mut self, edge: ResizeEdge, context: &egui::Context) {
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


    pub(crate) fn update_expanded_resize(&mut self, context: &egui::Context) {
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


    pub(crate) fn finish_expanded_resize(&mut self, context: &egui::Context) {
        self.expanded_resize_drag = None;
        self.expanded_size =
            clamp_expanded_size_to_monitor(self.expanded_size, self.last_known_monitor_rect);
        self.last_hovered_at = Instant::now();
        self.apply_dock_geometry(context);
        self.save_layout_settings();
    }


    pub(crate) fn begin_window_resize(&mut self, edge: ResizeEdge, context: &egui::Context) {
        let start_window_rect = egui::Rect::from_min_size(self.last_known_position, self.window_size);
        self.window_resize_drag = Some(ResizeDrag {
            edge,
            start_window_rect,
        });
        self.last_status_message = "창 크기를 조절하는 중입니다.".to_owned();
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
    }


    pub(crate) fn update_window_resize(&mut self, context: &egui::Context) {
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


    pub(crate) fn finish_window_resize(&mut self, context: &egui::Context) {
        self.window_resize_drag = None;
        context.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.window_size));
        self.last_status_message = "창 크기를 저장했습니다.".to_owned();
        self.save_layout_settings();
    }


    pub(crate) fn current_layout(&self) -> LayoutSettings {
        LayoutSettings {
            expanded_size: self.expanded_size,
            window_size: self.window_size,
            dock_edge: self.dock_edge,
            monitors: self.monitor_dock_states.clone(),
        }
    }


    pub(crate) fn record_current_monitor_state(&mut self) {
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


    pub(crate) fn restore_monitor_state_for_current(&mut self) {
        let key = monitor_key(self.last_known_monitor_rect);
        if let Some(state) = self.monitor_dock_states.get(&key).copied() {
            self.expanded_size =
                clamp_expanded_size_to_monitor(state.expanded_size, self.last_known_monitor_rect);
        }
    }


    pub(crate) fn save_layout_settings(&mut self) {
        self.record_current_monitor_state();
        let layout = self.current_layout();
        if let Err(error) =
            save_tabs_to_configuration(&self.configuration_path, &self.tabs, &layout, false)
        {
            self.last_status_message = format!("창 크기 저장 실패: {error}");
            log_event(&format!("레이아웃 저장 실패: {error}"));
        }
    }


    pub(crate) fn docked_window_size(&self, is_expanded: bool) -> egui::Vec2 {
        if is_expanded {
            self.expanded_size
        } else {
            get_window_size(self.dock_edge, false)
        }
    }


    pub(crate) fn apply_dock_geometry(&self, context: &egui::Context) {
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


    pub(crate) fn dock_to_edge_at(
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


    pub(crate) fn add_tab(&mut self) {
        let tab_name = format!("탭 {}", self.next_tab_number);
        self.next_tab_number += 1;
        self.tabs.push(QuickDockTab::new(tab_name, Vec::new()));
        self.active_tab_index = self.tabs.len().saturating_sub(1);
        self.renaming_tab_index = Some(self.active_tab_index);
        self.renaming_focus_pending = true;
    }


    pub(crate) fn close_active_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }

        self.tabs.remove(self.active_tab_index);
        self.active_tab_index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        self.renaming_tab_index = None;
        self.renaming_focus_pending = false;
    }


    pub(crate) fn active_tab_name(&self) -> &str {
        self.active_tab().name.as_str()
    }


    pub(crate) fn active_tab(&self) -> &QuickDockTab {
        &self.tabs[self.active_tab_index.min(self.tabs.len().saturating_sub(1))]
    }


    pub(crate) fn active_tab_mut(&mut self) -> &mut QuickDockTab {
        let index = self.active_tab_index.min(self.tabs.len().saturating_sub(1));
        &mut self.tabs[index]
    }


    pub(crate) fn open_settings_editor(&mut self) {
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


    pub(crate) fn save_settings_editor(&mut self) {
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


    pub(crate) fn cancel_settings_editor(&mut self) {
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


    pub(crate) fn add_editable_item(&mut self, kind: ActionKind) {
        self.active_tab_mut()
            .editable_items
            .push(EditableActionItem::blank(kind));
    }


    pub(crate) fn close_related_explorer_windows(&mut self) {
        match run_explorer_cleanup() {
            Ok(()) => {
                self.last_status_message = "중복/하위 탐색기 창을 정리했습니다.".to_owned();
            }
            Err(error) => {
                self.last_status_message = format!("탐색기 정리 실패: {error}");
            }
        }
    }


    pub(crate) fn handle_button_action_request(&mut self, request: ButtonActionRequest) {
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


    pub(crate) fn commit_active_items(&mut self, status: impl Into<String>) {
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


    pub(crate) fn open_settings_editor_at(&mut self, index: usize) {
        self.open_settings_editor();
        self.editor_focus_index = Some(index);
        self.last_status_message = format!("{}번째 항목을 편집합니다.", index + 1);
    }


    pub(crate) fn run_item_as_admin(&mut self, item: &ActionItem) {
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


    pub(crate) fn open_item_location(&mut self, item: &ActionItem) {
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


    pub(crate) fn copy_item_path(&mut self, item: &ActionItem) {
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


    pub(crate) fn pointer_to_monitor_position(&self, pointer_position: egui::Pos2) -> egui::Pos2 {
        self.last_known_inner_position + pointer_position.to_vec2()
    }


    pub(crate) fn execute_action(&mut self, item: &ActionItem) {
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


    pub(crate) fn begin_copy(&mut self, name: &str, text: &str) {
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


    pub(crate) fn confirm_pending_input_copy(&mut self) {
        let Some(pending) = self.pending_input_copy.take() else {
            return;
        };
        let selection_text = self.selection_text_for_template(&pending.template);
        let expanded = expand_copy_template(&pending.template, &pending.fields, &selection_text);
        self.set_clipboard_text(&pending.name, &expanded);
    }


    pub(crate) fn selection_text_for_template(&self, template: &str) -> String {
        if template.contains("{selection}") {
            self.capture_selection_text().unwrap_or_default()
        } else {
            String::new()
        }
    }


    pub(crate) fn capture_selection_text(&self) -> Option<String> {
        capture_selection_text(self.last_external_foreground_window)
    }


    pub(crate) fn set_clipboard_text(&mut self, name: &str, text: &str) {
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


    pub(crate) fn ensure_tray_icon(&mut self, context: &egui::Context) {
        #[cfg(target_os = "windows")]
        {
            if self.tray_state.is_some() || self.tray_attempted {
                return;
            }
            self.tray_attempted = true;
            match build_tray_state(
                self.autostart_enabled,
                context.clone(),
                self.tray_commands.clone(),
            ) {
                Ok(state) => self.tray_state = Some(state),
                Err(error) => log_event(&format!("트레이 아이콘 생성 실패: {error}")),
            }
        }

        #[cfg(not(target_os = "windows"))]
        let _ = context;
    }


    pub(crate) fn poll_tray_events(&mut self, context: &egui::Context) {
        #[cfg(target_os = "windows")]
        {
            // 핸들러가 쌓아 둔 명령을 비운다. (메뉴/아이콘 이벤트는 채널이 아니라
            // set_event_handler로 받아 tray_commands 큐에 들어온다.)
            let commands: Vec<TrayCommand> = match self.tray_commands.lock() {
                Ok(mut queue) => queue.drain(..).collect(),
                Err(_) => return,
            };

            for command in commands {
                match command {
                    TrayCommand::Open => self.bring_to_front(context),
                    TrayCommand::Settings => {
                        self.bring_to_front(context);
                        if !self.is_settings_editor_open {
                            self.open_settings_editor();
                        }
                    }
                    TrayCommand::ToggleAutostart => {
                        self.toggle_autostart();
                        if let Some(state) = &self.tray_state {
                            state.autostart_item.set_checked(self.autostart_enabled);
                        }
                    }
                    TrayCommand::Quit => {
                        context.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        let _ = context;
    }


    pub(crate) fn bring_to_front(&mut self, context: &egui::Context) {
        self.is_expanded = true;
        if self.is_docked {
            self.apply_dock_geometry(context);
        }
        context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        context.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.last_hovered_at = Instant::now();
    }


    pub(crate) fn toggle_autostart(&mut self) {
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


    pub(crate) fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
        if self.palette_open {
            self.palette_query.clear();
            self.palette_selected = 0;
            self.palette_focus_pending = true;
            self.last_status_message = "검색: 이름·내용·탭".to_owned();
        }
    }


    pub(crate) fn record_recent(&mut self, item: &ActionItem) {
        self.recent_items
            .retain(|existing| existing.kind() != item.kind() || existing.name() != item.name());
        self.recent_items.insert(0, item.clone());
        self.recent_items.truncate(8);
    }


    pub(crate) fn build_palette_entries(&self) -> Vec<PaletteEntry> {
        let query = self.palette_query.trim().to_lowercase();
        let mut entries = Vec::new();

        if query.is_empty() {
            for item in &self.recent_items {
                entries.push(PaletteEntry {
                    item: item.clone(),
                    tab_index: None,
                    tab_label: "최근".to_owned(),
                });
            }
            for (tab_index, tab) in self.tabs.iter().enumerate() {
                for item in &tab.items {
                    entries.push(PaletteEntry {
                        item: item.clone(),
                        tab_index: Some(tab_index),
                        tab_label: tab.name.clone(),
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
                        });
                    }
                }
            }
        }

        entries.truncate(80);
        entries
    }


    pub(crate) fn execute_palette_entry(&mut self, entry: PaletteEntry) {
        self.palette_open = false;
        if let Some(tab_index) = entry.tab_index {
            self.active_tab_index = tab_index.min(self.tabs.len().saturating_sub(1));
        }
        self.record_recent(&entry.item);
        self.execute_action(&entry.item);
    }


    pub(crate) fn show_toast(&mut self, message: impl Into<String>, is_error: bool) {
        self.toast_message = Some(message.into());
        self.toast_started_at = Instant::now();
        self.toast_is_error = is_error;
    }


    pub(crate) fn run_application(&mut self, name: &str, command: &str, arguments: &[String]) {
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


    pub(crate) fn open_path(&mut self, name: &str, path: &str) {
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
        self.ensure_tray_icon(&context);
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
