use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::platform::show_main_window;

#[cfg(target_os = "windows")]
pub(crate) struct TrayState {
    pub(crate) _icon: tray_icon::TrayIcon,
    pub(crate) autostart_item: tray_icon::menu::CheckMenuItem,
}

/// 트레이 메뉴/아이콘 핸들러가 update 루프에 전달하는 명령.
///
/// 창을 숨기면 eframe가 `ui()`를 더 이상 호출하지 않으므로(메모리: 트레이는 창을 숨기지 않음 참고)
/// 트레이 이벤트는 채널 폴링 대신 `set_event_handler`로 받아 이 큐에 쌓고, 메인 스레드 메시지
/// 펌프에서 직접 창을 다시 표시해 update 루프를 깨운 뒤 처리한다.
#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
pub(crate) enum TrayCommand {
    Open,
    Settings,
    ToggleAutostart,
    Quit,
}

#[cfg(target_os = "windows")]
pub(crate) fn build_tray_state(
    autostart_enabled: bool,
    context: egui::Context,
    commands: Arc<Mutex<VecDeque<TrayCommand>>>,
) -> Result<TrayState, String> {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    // 메뉴/아이콘 이벤트를 update 루프 밖(메인 스레드 메시지 펌프)에서 받는다.
    // 창이 숨겨져 ui()가 멈춰도 핸들러는 호출되므로, 명령을 큐에 넣고 창을 다시 표시해
    // update 루프를 깨운다. (request_repaint는 창이 보일 때의 즉시 처리를 위한 보조 수단)
    let menu_commands = commands.clone();
    let menu_context = context.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let command = if event.id == "tray_open" {
            TrayCommand::Open
        } else if event.id == "tray_settings" {
            TrayCommand::Settings
        } else if event.id == "tray_autostart" {
            TrayCommand::ToggleAutostart
        } else if event.id == "tray_quit" {
            TrayCommand::Quit
        } else {
            return;
        };
        if let Ok(mut queue) = menu_commands.lock() {
            queue.push_back(command);
        }
        show_main_window();
        menu_context.request_repaint();
    }));

    let icon_commands = commands;
    let icon_context = context;
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        // 왼쪽 버튼을 뗄 때 한 번만 창을 복원한다.
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            if let Ok(mut queue) = icon_commands.lock() {
                queue.push_back(TrayCommand::Open);
            }
            show_main_window();
            icon_context.request_repaint();
        }
    }));

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
pub(crate) fn build_tray_icon_image() -> Result<tray_icon::Icon, String> {
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
