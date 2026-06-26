use std::time::Duration;

use eframe::egui;

use crate::config::log_event;
use crate::constants::*;
use crate::model::*;
use crate::layout::*;
use crate::widgets::*;
use crate::app::QuickDockApplication;

impl QuickDockApplication {
    pub(crate) fn show_collapsed_user_interface(&mut self, ui: &mut egui::Ui) {
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


    pub(crate) fn show_drag_preview_user_interface(&mut self, ui: &mut egui::Ui) {
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


    pub(crate) fn show_expanded_user_interface(&mut self, ui: &mut egui::Ui) {
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


    pub(crate) fn show_expanded_resize_handles(&mut self, ui: &mut egui::Ui, frame_rect: egui::Rect) {
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


    pub(crate) fn show_action_button(
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


    pub(crate) fn show_header(&mut self, ui: &mut egui::Ui) {
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

                let close_tooltip = if self.is_settings_editor_open {
                    "저장하고 닫기"
                } else {
                    "닫기"
                };
                let close_response = ui
                    .add_sized(
                        close_size,
                        egui::Button::new(egui::RichText::new("X").strong().size(14.0))
                            .fill(egui::Color32::TRANSPARENT),
                    )
                    .on_hover_text(close_tooltip);

                if close_response.clicked() {
                    self.handle_close_button(ui.ctx());
                }
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

                if toolbar_icon_button(ui, "검색 (Ctrl+Space)", self.palette_open, draw_search_icon)
                    .clicked()
                {
                    self.toggle_palette();
                }

                if toolbar_icon_button(ui, "새 탭", false, draw_plus_icon).clicked() {
                    self.add_tab();
                }

                if toolbar_icon_button(ui, "다시 읽기", false, draw_refresh_icon).clicked() {
                    self.reload_configuration();
                }

                if toolbar_icon_button(ui, "탐색기 정리", false, draw_tidy_icon).clicked() {
                    self.close_related_explorer_windows();
                }

                let autostart_tooltip = if self.autostart_enabled {
                    "자동 실행: 켜짐 (클릭하여 끄기)"
                } else {
                    "자동 실행: 꺼짐 (클릭하여 켜기)"
                };
                if toolbar_icon_button(ui, autostart_tooltip, self.autostart_enabled, draw_power_icon)
                    .clicked()
                {
                    self.toggle_autostart();
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                if toolbar_icon_button(ui, "설정", self.is_settings_editor_open, draw_gear_icon)
                    .clicked()
                {
                    if self.is_settings_editor_open {
                        self.cancel_settings_editor();
                    } else {
                        self.open_settings_editor();
                    }
                }
            });
        });
    }


    pub(crate) fn handle_close_button(&mut self, context: &egui::Context) {
        if self.is_settings_editor_open {
            // 설정 편집 중 X = 저장하고 닫기 (검증 실패 시 편집 화면 유지)
            self.save_settings_editor();
        } else if self.palette_open {
            self.palette_open = false;
        } else if self.pending_input_copy.is_some() {
            self.pending_input_copy = None;
            self.last_status_message = "복사를 취소했습니다.".to_owned();
        } else {
            self.hide_to_tray(context);
        }
    }

    /// X 버튼: 종료하지 않고 트레이로 최소화한다. 트레이의 `종료`로만 완전히 끈다.

    pub(crate) fn hide_to_tray(&mut self, context: &egui::Context) {
        #[cfg(target_os = "windows")]
        {
            // 트레이로 숨기기 전에 트레이 아이콘이 살아 있는지 확인한다.
            // (없으면 다시 꺼낼 방법이 사라지므로 안전하게 그냥 종료한다.)
            self.ensure_tray_icon(context);
            if self.tray_state.is_some() {
                context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.last_status_message =
                    "트레이로 최소화했습니다. 트레이 아이콘에서 다시 열 수 있습니다.".to_owned();
                return;
            }
            // 트레이 아이콘이 없으면 다시 꺼낼 방법이 없으므로 안전하게 종료한다.
            log_event("트레이 아이콘이 없어 X를 종료로 처리");
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        #[cfg(not(target_os = "windows"))]
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }


    pub(crate) fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
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


    pub(crate) fn show_settings_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let save_button = egui::Button::new(
                egui::RichText::new("저장")
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(egui::Color32::from_rgb(46, 150, 95));
            if ui
                .add(save_button)
                .on_hover_text("저장하고 닫기 (제목줄 X도 동일)")
                .clicked()
            {
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


    pub(crate) fn show_pending_input_modal(&mut self, context: &egui::Context) {
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


    pub(crate) fn show_palette(&mut self, context: &egui::Context) {
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
                                let label = format!(
                                    "{} · {}",
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
                    egui::RichText::new("위/아래 이동 · Enter 실행 · Esc 닫기")
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


    pub(crate) fn show_toast_overlay(&mut self, context: &egui::Context) {
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

}
