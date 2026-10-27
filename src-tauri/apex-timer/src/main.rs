//! Tray helper that holds a timer resolution until stopped.
//!
//! Started by Apex Tune (or its logon task) as `apex-timer --resolution N`.
//! It runs without a window or admin rights, shows a tray icon, and exits
//! when Apex Tune signals its stop event or you pick Stop in the tray menu.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    if let Err(e) = app::run() {
        app::show_error(&e);
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    match apex_timer::parse_args(std::env::args()) {
        Ok(units) => eprintln!(
            "apex-timer would hold {} ms, but it only runs on Windows",
            apex_timer::to_ms(units)
        ),
        Err(e) => eprintln!("{e}"),
    }
    std::process::exit(1);
}

#[cfg(windows)]
mod app {
    use apex_timer::{MUTEX_NAME, STOP_EVENT_NAME, to_ms};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, WAIT_OBJECT_0,
    };
    use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, INFINITE};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MB_ICONERROR, MB_OK, MSG, MessageBoxW, MsgWaitForMultipleObjects,
        PM_REMOVE, PeekMessageW, QS_ALLINPUT, SW_SHOWNORMAL, TranslateMessage, WM_QUIT,
    };
    use windows::core::{HSTRING, PCWSTR};

    pub fn show_error(message: &str) {
        // SAFETY: both strings are valid for the call.
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(message),
                &HSTRING::from("Apex Tune timer"),
                MB_OK | MB_ICONERROR,
            )
        };
    }

    pub fn run() -> Result<(), String> {
        let units = apex_timer::parse_args(std::env::args())?;

        // SAFETY: valid name; the handle lives until the process exits.
        let mutex = unsafe { CreateMutexW(None, true, &HSTRING::from(MUTEX_NAME)) }
            .map_err(|e| e.message())?;
        // SAFETY: reads the thread's last error right after CreateMutexW.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // Another helper is already running; Apex Tune stops it before starting a new one.
            return Ok(());
        }
        // SAFETY: valid name; auto-reset, initially unsignalled.
        let stop = unsafe { CreateEventW(None, false, false, &HSTRING::from(STOP_EVENT_NAME)) }
            .map_err(|e| e.message())?;

        apex_timer::honor_timer_requests_when_hidden();
        let actual =
            apex_timer::request(units).ok_or("Windows refused the timer resolution request")?;

        let label = MenuItem::new(
            format!("Holding {:.3} ms timer resolution", to_ms(actual)),
            false,
            None,
        );
        let open = MenuItem::new("Open Apex Tune", true, None);
        let quit = MenuItem::new("Stop", true, None);
        let menu = Menu::new();
        menu.append_items(&[&label, &PredefinedMenuItem::separator(), &open, &quit])
            .map_err(|e| e.to_string())?;
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(format!("Apex Tune: timer at {:.3} ms", to_ms(actual)))
            .with_icon(icon())
            .build()
            .map_err(|e| e.to_string())?;

        loop {
            // SAFETY: one valid event handle; wakes on the event or any window message.
            let wait =
                unsafe { MsgWaitForMultipleObjects(Some(&[stop]), false, INFINITE, QS_ALLINPUT) };
            if wait == WAIT_OBJECT_0 {
                break;
            }
            if !pump_messages() {
                break;
            }
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == quit.id() {
                    break;
                }
                if event.id == open.id() {
                    open_apex_tune();
                }
            }
        }

        apex_timer::release(units);
        // SAFETY: handles created above, closed once.
        unsafe {
            let _ = CloseHandle(stop);
            let _ = CloseHandle(mutex);
        }
        Ok(())
    }

    /// Dispatches pending messages. Returns false on WM_QUIT.
    fn pump_messages() -> bool {
        let mut msg = MSG::default();
        // SAFETY: standard message pump on this thread's queue.
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            // SAFETY: `msg` came from PeekMessageW.
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        true
    }

    fn open_apex_tune() {
        let Some(exe) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("apex-tune.exe")))
        else {
            return;
        };
        // "open" lets Windows show the UAC prompt Apex Tune's manifest asks for.
        // SAFETY: valid strings for the call.
        unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("open"),
                &HSTRING::from(exe.as_os_str()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    /// A 32×32 ring in the Apex Tune accent, drawn in code so no image decoder is needed.
    fn icon() -> Icon {
        const SIZE: u32 = 32;
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
                let d = (dx * dx + dy * dy).sqrt();
                let on_ring = (10.0..=14.5).contains(&d);
                let on_hub = d <= 4.0;
                let (r, g, b, a) = if on_ring {
                    (0x7c, 0xc4, 0xfa, 255)
                } else if on_hub {
                    (0xe8, 0xed, 0xf3, 255)
                } else {
                    (0, 0, 0, 0)
                };
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
        Icon::from_rgba(rgba, SIZE, SIZE).expect("icon dimensions match the buffer")
    }
}
