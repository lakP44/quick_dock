use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app::TemplateInputField;
use crate::model::*;
#[cfg(target_os = "windows")]
use crate::platform::wide_null;
use crate::platform::{current_local_datetime, read_clipboard_text};

pub(crate) fn validate_editable_items_for_save(items: &[EditableActionItem]) -> Option<String> {
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

pub(crate) fn check_application_command(command: &str) -> ValidationMessage {
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

pub(crate) fn check_open_path(path: &str) -> ValidationMessage {
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

pub(crate) fn resolve_application_command_for_spawn(command: &str) -> Result<String, String> {
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

pub(crate) fn resolve_open_path_for_spawn(path: &str) -> Result<String, String> {
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
pub(crate) fn run_application_as_admin(command: &str, arguments: &[String]) -> Result<(), String> {
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
pub(crate) fn run_application_as_admin(_command: &str, _arguments: &[String]) -> Result<(), String> {
    Err("관리자 권한 실행은 Windows에서만 지원합니다.".to_owned())
}

pub(crate) fn reveal_in_file_explorer(path: &str) -> Result<(), String> {
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

pub(crate) fn normalize_command_text(text: &str) -> String {
    let expanded_text = expand_environment_variables(text);
    trim_wrapping_quotes(expanded_text.trim()).trim().to_owned()
}

pub(crate) fn looks_like_url(text: &str) -> bool {
    let lower_text = text.to_ascii_lowercase();
    lower_text.starts_with("http://")
        || lower_text.starts_with("https://")
        || lower_text.starts_with("mailto:")
}

pub(crate) fn describe_spawn_error(error: &std::io::Error, target: &str) -> String {
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
pub(crate) fn choose_executable_file() -> std::io::Result<Option<String>> {
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
pub(crate) fn choose_executable_file() -> std::io::Result<Option<String>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "파일 선택은 Windows에서만 지원합니다.",
    ))
}

pub(crate) fn run_explorer_cleanup() -> std::io::Result<()> {
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


#[cfg(target_os = "windows")]
pub(crate) fn find_application_command(command: &str) -> Option<PathBuf> {
    find_windows_application_command(command)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn find_application_command(command: &str) -> Option<PathBuf> {
    let path_value = env::var_os("PATH")?;

    for directory in env::split_paths(&path_value) {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub(crate) fn trim_wrapping_quotes(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

pub(crate) fn looks_like_path(command: &str) -> bool {
    command.contains('\\') || command.contains('/') || command.contains(':')
}

#[cfg(target_os = "windows")]
pub(crate) fn find_windows_application_command(command: &str) -> Option<PathBuf> {
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
pub(crate) fn windows_command_names(command: &str) -> Vec<String> {
    let path = Path::new(command);

    if path.extension().is_some() {
        vec![command.to_owned()]
    } else {
        vec![command.to_owned(), format!("{command}.exe")]
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn find_command_in_path(command_names: &[String]) -> Option<PathBuf> {
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
pub(crate) fn windows_application_search_directories(command: &str) -> Vec<PathBuf> {
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
pub(crate) fn known_windows_application_folder_names(command_stem: &str) -> Vec<String> {
    let mut folder_names = vec![command_stem.to_owned()];

    if command_stem.eq_ignore_ascii_case("notepad++") {
        folder_names.push("Notepad++".to_owned());
    }

    folder_names
}

pub(crate) fn extract_input_labels(template: &str) -> Vec<String> {
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

pub(crate) fn expand_copy_template(
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

pub(crate) fn expand_environment_variables(text: &str) -> String {
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
