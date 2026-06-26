use std::sync::Arc;

use std::fs;

use eframe::egui;

use crate::model::*;
use crate::commands::check_application_command;
use crate::commands::check_open_path;
use crate::commands::choose_executable_file;

pub(crate) fn format_action_button_label(item: &ActionItem) -> String {
    let name = shorten_text(item.name(), 30);
    match item {
        ActionItem::CopyText { .. } => format!("복사 · {name}"),
        ActionItem::RunApplication { .. } => format!("실행 · {name}"),
        ActionItem::OpenPath { .. } => format!("열기 · {name}"),
    }
}

pub(crate) fn toolbar_icon_button(
    ui: &mut egui::Ui,
    tooltip: &str,
    active: bool,
    draw: fn(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(32.0, 28.0), egui::Sense::click());

    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(72, 160, 148));
    } else if response.hovered() {
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(222, 233, 238));
    }

    let icon_color = if active {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(40, 60, 72)
    };
    draw(painter, rect.shrink(6.0), icon_color);

    if response.hovered() {
        ui.output_mut(|output| output.cursor_icon = egui::CursorIcon::PointingHand);
    }

    response.on_hover_text(tooltip)
}

pub(crate) fn arc_point(center: egui::Pos2, radius: f32, degrees: f32) -> egui::Pos2 {
    let radians = degrees.to_radians();
    egui::pos2(
        center.x + radius * radians.cos(),
        center.y + radius * radians.sin(),
    )
}

pub(crate) fn arc_points(
    center: egui::Pos2,
    radius: f32,
    start_degrees: f32,
    end_degrees: f32,
    segments: usize,
) -> Vec<egui::Pos2> {
    (0..=segments)
        .map(|index| {
            let fraction = index as f32 / segments as f32;
            let degrees = start_degrees + (end_degrees - start_degrees) * fraction;
            arc_point(center, radius, degrees)
        })
        .collect()
}

pub(crate) fn draw_search_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.8, color);
    let radius = rect.width().min(rect.height()) * 0.32;
    let center = rect.min + egui::vec2(rect.width() * 0.40, rect.height() * 0.40);
    painter.circle_stroke(center, radius, stroke);
    let handle_start = center + egui::vec2(radius * 0.7, radius * 0.7);
    let handle_end = rect.max - egui::vec2(rect.width() * 0.06, rect.height() * 0.06);
    painter.line_segment([handle_start, handle_end], egui::Stroke::new(2.2, color));
}

pub(crate) fn draw_plus_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.2, color);
    let center = rect.center();
    let arm = rect.width().min(rect.height()) * 0.42;
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - arm),
            egui::pos2(center.x, center.y + arm),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x - arm, center.y),
            egui::pos2(center.x + arm, center.y),
        ],
        stroke,
    );
}

pub(crate) fn draw_refresh_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.9, color);
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.40;
    painter.add(egui::Shape::line(
        arc_points(center, radius, -50.0, 210.0, 24),
        stroke,
    ));

    // 화살촉: 호의 끝(210°)에서 안/밖으로 짧은 선
    let tip = arc_point(center, radius, 210.0);
    painter.line_segment([tip, arc_point(center, radius * 0.70, 196.0)], stroke);
    painter.line_segment([tip, arc_point(center, radius * 1.30, 196.0)], stroke);
}

pub(crate) fn draw_tidy_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.7, color);
    let size = egui::vec2(rect.width() * 0.56, rect.height() * 0.56);
    let back = egui::Rect::from_min_size(rect.min + egui::vec2(rect.width() * 0.30, 0.0), size);
    let front = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, rect.height() * 0.30), size);
    painter.rect_stroke(back, 2.0, stroke, egui::StrokeKind::Inside);
    painter.rect_filled(front, 2.0, egui::Color32::from_rgb(242, 246, 248));
    painter.rect_stroke(front, 2.0, stroke, egui::StrokeKind::Inside);
}

pub(crate) fn draw_power_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.9, color);
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.36;
    // 윗부분이 열린 원 (전원 기호)
    painter.add(egui::Shape::line(
        arc_points(center, radius, -60.0, 240.0, 22),
        stroke,
    ));
    painter.line_segment(
        [
            egui::pos2(center.x, rect.top()),
            egui::pos2(center.x, center.y - radius * 0.1),
        ],
        stroke,
    );
}

pub(crate) fn draw_gear_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.7, color);
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.30;
    painter.circle_stroke(center, radius, stroke);
    painter.circle_stroke(center, radius * 0.42, stroke);
    for tooth in 0..6 {
        let degrees = tooth as f32 * 60.0;
        painter.line_segment(
            [
                arc_point(center, radius, degrees),
                arc_point(center, radius * 1.5, degrees),
            ],
            egui::Stroke::new(2.0, color),
        );
    }
}

pub(crate) fn show_copy_text_preview(ui: &mut egui::Ui, name: &str, text: &str) {
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

pub(crate) fn show_editable_action_item(
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

pub(crate) fn show_validation_message(ui: &mut egui::Ui, message: &ValidationMessage) {
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

pub(crate) fn validation_color(ui: &egui::Ui, severity: ValidationSeverity) -> egui::Color32 {
    match severity {
        ValidationSeverity::Ok => egui::Color32::from_rgb(45, 150, 95),
        ValidationSeverity::Warning => egui::Color32::from_rgb(188, 128, 35),
        ValidationSeverity::Error => ui.visuals().error_fg_color,
    }
}


pub(crate) fn shorten_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let shortened: String = chars.by_ref().take(max_chars).collect();

    if chars.next().is_some() {
        format!("{shortened}...")
    } else {
        shortened
    }
}

pub(crate) fn configure_system_fonts(context: &egui::Context) {
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

pub(crate) fn load_first_existing_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| fs::read(path).ok())
}

pub(crate) fn system_font_candidates() -> &'static [&'static str] {
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
