//! Positions the Windows frame on its launch display.

use rune::{WindowHandle, raw_window_handle::RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::{
        Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    },
    UI::WindowsAndMessaging::{GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos},
};

pub fn center(window: &WindowHandle) {
    let _ = window.with_raw_handle(|handle| {
        let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
        let hwnd = handle.hwnd.get() as HWND;
        // Rune calls this on the UI thread while the borrowed HWND is alive.
        unsafe {
            let mut frame: RECT = std::mem::zeroed();
            let mut monitor: MONITORINFO = std::mem::zeroed();
            monitor.cbSize = size_of::<MONITORINFO>() as u32;
            let display = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if GetWindowRect(hwnd, &mut frame) == 0 || GetMonitorInfoW(display, &mut monitor) == 0 {
                return;
            }

            // DWM excludes the invisible resize borders from the visible frame.
            let mut visible = frame;
            let status = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                (&raw mut visible).cast(),
                size_of::<RECT>() as u32,
            );
            if status < 0 || visible.right <= visible.left || visible.bottom <= visible.top {
                visible = frame;
            }
            let area = monitor.rcMonitor;
            let work = monitor.rcWork;
            let x = centered_axis(area.left, area.right, work.left, work.right, visible.right - visible.left);
            let y = centered_axis(area.top, area.bottom, work.top, work.bottom, visible.bottom - visible.top);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x + frame.left - visible.left,
                y + frame.top - visible.top,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    });
}

fn centered_axis(start: i32, end: i32, work_start: i32, work_end: i32, length: i32) -> i32 {
    let center = start + (end - start - length).div_euclid(2);
    center.clamp(work_start, (work_end - length).max(work_start))
}
