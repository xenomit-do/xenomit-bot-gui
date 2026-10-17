//! Minimize-to-tray (Windows): hides the main window and parks a
//! notification-area icon owned by a message-only window on its own thread.
//! Left-click or double-click restores; right-click offers Show / Exit.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, HWND_MESSAGE, IDI_APPLICATION, LoadIconW,
    MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SW_HIDE, SW_RESTORE, SW_SHOW,
    SetForegroundWindow, ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSW,
};

const WM_TRAY: u32 = WM_APP + 1;
const CMD_SHOW: usize = 1;
const CMD_EXIT: usize = 2;

static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data
}

fn restore_main(tray: HWND) {
    let main = MAIN_WINDOW.load(Ordering::Acquire) as HWND;
    unsafe {
        ShowWindow(main, SW_SHOW);
        ShowWindow(main, SW_RESTORE);
        SetForegroundWindow(main);
        DestroyWindow(tray);
    }
}

unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_TRAY => {
            match (lparam & 0xffff) as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => restore_main(hwnd),
                WM_RBUTTONUP => unsafe {
                    let menu = CreatePopupMenu();
                    AppendMenuW(menu, MF_STRING, CMD_SHOW, wide("Show Xenomit Bot").as_ptr());
                    AppendMenuW(menu, MF_STRING, CMD_EXIT, wide("Exit").as_ptr());
                    let mut cursor = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut cursor);
                    // The menu only dismisses on an outside click when its
                    // owner is the foreground window.
                    SetForegroundWindow(hwnd);
                    let command = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        cursor.x,
                        cursor.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    ) as usize;
                    DestroyMenu(menu);
                    match command {
                        CMD_SHOW => restore_main(hwnd),
                        CMD_EXIT => {
                            let main = MAIN_WINDOW.load(Ordering::Acquire) as HWND;
                            ShowWindow(main, SW_SHOW);
                            PostMessageW(main, WM_CLOSE, 0, 0);
                            DestroyWindow(hwnd);
                        }
                        _ => {}
                    }
                },
                _ => {}
            }
            0
        }
        WM_DESTROY => unsafe {
            Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
            PostQuitMessage(0);
            0
        },
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// Hide `main` and show a tray icon that brings it back. A second call
/// while the icon is already parked does nothing.
pub fn hide_to_tray(main: isize) -> Result<(), String> {
    if ACTIVE.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    MAIN_WINDOW.store(main, Ordering::Release);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::spawn(move || unsafe {
        let class_name = wide("XenomitBotTray");
        let instance = GetModuleHandleW(std::ptr::null());
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(tray_proc);
        class.hInstance = instance;
        class.lpszClassName = class_name.as_ptr();
        // Registering twice (after a restore) fails harmlessly.
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            ACTIVE.store(false, Ordering::Release);
            let _ = ready_tx.send(Err("Could not create the tray window.".into()));
            return;
        }
        let mut data = icon_data(hwnd);
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        for (slot, unit) in data.szTip.iter_mut().zip(wide("Xenomit Bot")) {
            *slot = unit;
        }
        if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
            DestroyWindow(hwnd);
            ACTIVE.store(false, Ordering::Release);
            let _ = ready_tx.send(Err("Could not add the tray icon.".into()));
            return;
        }
        ShowWindow(MAIN_WINDOW.load(Ordering::Acquire) as HWND, SW_HIDE);
        let _ = ready_tx.send(Ok(()));
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        ACTIVE.store(false, Ordering::Release);
    });
    ready_rx
        .recv()
        .unwrap_or_else(|_| Err("The tray thread stopped unexpectedly.".into()))
}
