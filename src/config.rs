use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::env;

use eframe::egui;

use crate::constants::*;
use crate::model::*;
use crate::layout::*;
use crate::platform::current_local_datetime;

pub(crate) fn get_configuration_path() -> PathBuf {
    configuration_directory().join(CONFIGURATION_FILE_NAME)
}

pub(crate) fn get_log_path() -> PathBuf {
    configuration_directory().join(LOG_FILE_NAME)
}

pub(crate) fn configuration_directory() -> PathBuf {
    let executable_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let executable_directory = executable_path.parent().unwrap_or_else(|| Path::new("."));
    executable_directory.join(CONFIGURATION_DIRECTORY_NAME)
}

pub(crate) fn log_event(message: &str) {
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

pub(crate) fn load_tabs_from_configuration(
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

pub(crate) fn ensure_configuration_file(configuration_path: &Path) -> bool {
    if configuration_path.exists() {
        return false;
    }

    if let Some(parent) = configuration_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::write(configuration_path, get_default_ini_text()).is_ok()
}

pub(crate) fn save_tabs_to_configuration(
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

pub(crate) fn backup_configuration_file(configuration_path: &Path) -> std::io::Result<()> {
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

pub(crate) fn serialize_tabs_to_ini(tabs: &[QuickDockTab], layout: &LayoutSettings) -> String {
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

pub(crate) fn escape_ini_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

pub(crate) fn parse_layout_from_ini(configuration_text: &str) -> LayoutSettings {
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

pub(crate) fn parse_f32_value(properties: &BTreeMap<String, String>, key: &str) -> Option<f32> {
    optional_ini_value(properties, key).and_then(|value| value.parse::<f32>().ok())
}


pub(crate) fn parse_tabs_from_ini(configuration_text: &str) -> Result<Vec<QuickDockTab>, String> {
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

pub(crate) fn parse_ini_sections(
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

pub(crate) fn parse_items_from_sections(
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

pub(crate) fn parse_action_item_from_properties(
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

pub(crate) fn required_ini_value(
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

pub(crate) fn optional_ini_value(properties: &BTreeMap<String, String>, key: &str) -> Option<String> {
    properties
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
}

pub(crate) fn parse_argument_list(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|argument| !argument.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn normalize_ini_value(value: &str) -> String {
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

pub(crate) fn unescape_ini_value(value: &str) -> String {
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

pub(crate) fn get_default_ini_text() -> &'static str {
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

pub(crate) fn get_default_items() -> Vec<ActionItem> {
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

