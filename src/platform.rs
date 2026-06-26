#[cfg(target_os = "windows")]
use std::env;
#[cfg(target_os = "windows")]
use std::time::Duration;

use eframe::egui;

use arboard::Clipboard;

use crate::constants::AUTOSTART_VALUE_NAME;

#[cfg(target_os = "windows")]
pub(crate) struct SingleInstanceGuard {
    pub(crate) handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl SingleInstanceGuard {
    pub(crate) fn acquire() -> Option<Self> {
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
pub(crate) fn focus_existing_instance_window() {
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

/// 트레이에서 창을 복원한다. 트레이 이벤트 핸들러는 메인 스레드 메시지 펌프에서
/// 실행되므로, 창이 `Visible(false)`로 숨겨져 eframe의 update 루프가 멈춰 있어도
/// 여기서 직접 창을 다시 표시하면 WM_PAINT가 발생해 update 루프가 깨어난다.
#[cfg(target_os = "windows")]
pub(crate) fn show_main_window() {
    use std::ffi::OsStr;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_SHOW,
    };

    let title = wide_null(OsStr::new("Quick Dock"));
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if !window.is_null() {
            ShowWindow(window, SW_SHOW);
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
pub(crate) fn wide_null(text: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
pub(crate) struct SingleInstanceGuard;

#[cfg(not(target_os = "windows"))]
impl SingleInstanceGuard {
    pub(crate) fn acquire() -> Option<Self> {
        Some(Self)
    }
}

pub(crate) fn is_primary_mouse_button_down() -> Option<bool> {
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

pub(crate) fn get_global_cursor_position(pixels_per_point: f32) -> Option<egui::Pos2> {
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

#[cfg(target_os = "windows")]
pub(crate) fn autostart_run_subkey() -> Vec<u16> {
    use std::ffi::OsStr;
    wide_null(OsStr::new(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    ))
}

#[cfg(target_os = "windows")]
pub(crate) fn is_autostart_enabled() -> bool {
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
pub(crate) fn is_autostart_enabled() -> bool {
    false
}

#[cfg(target_os = "windows")]
pub(crate) fn set_autostart(enabled: bool) -> Result<(), String> {
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
pub(crate) fn set_autostart(_enabled: bool) -> Result<(), String> {
    Err("자동 실행은 Windows에서만 지원합니다.".to_owned())
}

pub(crate) fn read_clipboard_text() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}

pub(crate) fn current_local_datetime() -> (i32, u32, u32, u32, u32, u32) {
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
pub(crate) fn civil_datetime_from_unix(total_seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
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

pub(crate) fn current_external_foreground_window() -> Option<isize> {
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

pub(crate) fn capture_selection_text(previous_window: Option<isize>) -> Option<String> {
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
pub(crate) unsafe fn send_copy_command() {
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
