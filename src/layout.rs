use std::collections::BTreeMap;

use eframe::egui;

use crate::constants::*;
use crate::model::DockEdge;

#[derive(Debug, Clone, Copy)]
pub(crate) enum DragDropTarget {
    Window {
        monitor_rect: egui::Rect,
    },
    Dock {
        edge: DockEdge,
        monitor_rect: egui::Rect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeEdge {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    pub(crate) fn cursor_icon(self) -> egui::CursorIcon {
        match self {
            ResizeEdge::TopLeft => egui::CursorIcon::ResizeNorthWest,
            ResizeEdge::TopRight => egui::CursorIcon::ResizeNorthEast,
            ResizeEdge::BottomLeft => egui::CursorIcon::ResizeSouthWest,
            ResizeEdge::BottomRight => egui::CursorIcon::ResizeSouthEast,
        }
    }

    pub(crate) fn affects_left(self) -> bool {
        matches!(self, ResizeEdge::TopLeft | ResizeEdge::BottomLeft)
    }

    pub(crate) fn affects_right(self) -> bool {
        matches!(self, ResizeEdge::TopRight | ResizeEdge::BottomRight)
    }

    pub(crate) fn affects_top(self) -> bool {
        matches!(self, ResizeEdge::TopLeft | ResizeEdge::TopRight)
    }

    pub(crate) fn affects_bottom(self) -> bool {
        matches!(self, ResizeEdge::BottomLeft | ResizeEdge::BottomRight)
    }

    pub(crate) fn id_salt(self) -> &'static str {
        match self {
            ResizeEdge::TopLeft => "top_left",
            ResizeEdge::TopRight => "top_right",
            ResizeEdge::BottomLeft => "bottom_left",
            ResizeEdge::BottomRight => "bottom_right",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResizeDrag {
    pub(crate) edge: ResizeEdge,
    pub(crate) start_window_rect: egui::Rect,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MonitorDockState {
    pub(crate) edge: DockEdge,
    pub(crate) anchor: egui::Pos2,
    pub(crate) expanded_size: egui::Vec2,
}

#[derive(Debug, Clone)]
pub(crate) struct LayoutSettings {
    pub(crate) expanded_size: egui::Vec2,
    pub(crate) window_size: egui::Vec2,
    pub(crate) dock_edge: DockEdge,
    pub(crate) monitors: BTreeMap<String, MonitorDockState>,
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

pub(crate) fn get_window_size(dock_edge: DockEdge, is_expanded: bool) -> egui::Vec2 {
    if is_expanded {
        return egui::vec2(EXPANDED_WIDTH, EXPANDED_HEIGHT);
    }

    match dock_edge {
        DockEdge::Left | DockEdge::Right => egui::vec2(COLLAPSED_THICKNESS, COLLAPSED_LENGTH),
        DockEdge::Top | DockEdge::Bottom => egui::vec2(COLLAPSED_LENGTH, COLLAPSED_THICKNESS),
    }
}

pub(crate) fn default_expanded_size() -> egui::Vec2 {
    egui::vec2(EXPANDED_WIDTH, EXPANDED_HEIGHT)
}

pub(crate) fn clamp_expanded_size(size: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        size.x.clamp(MIN_EXPANDED_WIDTH, MAX_EXPANDED_WIDTH),
        size.y.clamp(MIN_EXPANDED_HEIGHT, MAX_EXPANDED_HEIGHT),
    )
}

pub(crate) fn clamp_expanded_size_to_monitor(size: egui::Vec2, monitor_rect: egui::Rect) -> egui::Vec2 {
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

pub(crate) fn resize_handle_rects(
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

pub(crate) fn clamp_resize_rect(
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

pub(crate) fn clamp_free_window_rect(
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

pub(crate) fn edge_from_monitor_position(
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

pub(crate) fn clamp_pos_to_rect(position: egui::Pos2, rect: egui::Rect) -> egui::Pos2 {
    egui::pos2(
        position.x.clamp(rect.min.x, rect.max.x),
        position.y.clamp(rect.min.y, rect.max.y),
    )
}

pub(crate) fn normal_window_position_for_cursor(
    cursor_position: egui::Pos2,
    pointer_offset: egui::Vec2,
    monitor_rect: egui::Rect,
    window_size: egui::Vec2,
) -> egui::Pos2 {
    clamp_window_position(cursor_position - pointer_offset, window_size, monitor_rect)
}

pub(crate) fn clamp_window_position(
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

pub(crate) fn distance_to_rect(position: egui::Pos2, rect: egui::Rect) -> f32 {
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


pub(crate) fn get_docked_position(
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

pub(crate) fn monitor_key(rect: egui::Rect) -> String {
    format!(
        "{}x{}+{}+{}",
        rect.width().round() as i32,
        rect.height().round() as i32,
        rect.min.x.round() as i32,
        rect.min.y.round() as i32,
    )
}
