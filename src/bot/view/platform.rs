//! Platform-specific helpers: process usage and client window visibility.

use super::*;

/// CPU% (normalized across all cores) and working-set bytes of this
/// process: `GetProcessTimes` deltas over the sample interval, plus
/// `K32GetProcessMemoryInfo` for the working set.
#[cfg(windows)]
pub(super) fn process_usage(
    previous: Option<(Instant, u64)>,
) -> (Option<(Instant, u64)>, f64, u64) {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    unsafe {
        let process = GetCurrentProcess();
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
            return (previous, 0.0, 0);
        }
        let ticks = |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
        let total = ticks(kernel) + ticks(user);
        let now = Instant::now();
        let cpu = match previous {
            Some((at, last)) if total >= last => {
                let wall = now.duration_since(at).as_secs_f64();
                let cores = std::thread::available_parallelism()
                    .map(|n| n.get() as f64)
                    .unwrap_or(1.0);
                if wall > 0.0 {
                    ((total - last) as f64 / 10_000_000.0) / (wall * cores) * 100.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let mem = if K32GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0 {
            counters.WorkingSetSize as u64
        } else {
            0
        };
        (Some((now, total)), cpu.clamp(0.0, 100.0), mem)
    }
}

/// Working-set size of another process, 0 when it cannot be queried.
#[cfg(windows)]
pub(super) fn process_memory(pid: u32) -> u64 {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return 0;
        }
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let mem = if K32GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0 {
            counters.WorkingSetSize as u64
        } else {
            0
        };
        CloseHandle(process);
        mem
    }
}

#[cfg(not(windows))]
pub(super) fn process_memory(_pid: u32) -> u64 {
    0
}

#[cfg(not(windows))]
pub(super) fn process_usage(
    previous: Option<(Instant, u64)>,
) -> (Option<(Instant, u64)>, f64, u64) {
    (previous, 0.0, 0)
}

#[cfg(windows)]
pub(super) fn set_client_window_visible(pid: u32, visible: bool) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, SW_HIDE,
        SW_SHOWNORMAL, SetForegroundWindow, ShowWindow, WNDENUMPROC,
    };

    struct WindowSearch {
        pid: u32,
        hwnd: HWND,
    }

    unsafe extern "system" fn find_client_window(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = unsafe { &mut *(lparam as *mut WindowSearch) };
        let mut owner_pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut owner_pid) };
        if owner_pid != search.pid {
            return 1;
        }

        let title_length = unsafe { GetWindowTextLengthW(hwnd) };
        if title_length <= 0 {
            return 1;
        }
        let mut title = vec![0u16; title_length as usize + 1];
        let written = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
        if written <= 0 {
            return 1;
        }
        let title = String::from_utf16_lossy(&title[..written as usize]);
        if title.eq_ignore_ascii_case("Xenomit")
            || title.eq_ignore_ascii_case("Xenomit Flash Client")
        {
            search.hwnd = hwnd;
            0
        } else {
            1
        }
    }

    let mut search = WindowSearch {
        pid,
        hwnd: std::ptr::null_mut(),
    };
    let callback: WNDENUMPROC = Some(find_client_window);
    unsafe { EnumWindows(callback, &mut search as *mut WindowSearch as LPARAM) };
    if search.hwnd.is_null() {
        return Err("Could not find the Xenomit client window. It may still be starting.".into());
    }

    unsafe {
        ShowWindow(search.hwnd, if visible { SW_SHOWNORMAL } else { SW_HIDE });
        if visible {
            SetForegroundWindow(search.hwnd);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn set_client_window_visible(_pid: u32, _visible: bool) -> Result<(), String> {
    Err("Showing and hiding the game client is supported on Windows only.".into())
}
