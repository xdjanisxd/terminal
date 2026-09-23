//! Narrow platform services used by the terminal application.

use raw_window_handle::HasWindowHandle;
use std::io;

pub fn write_clipboard(text: &str, owner: Option<&impl HasWindowHandle>) -> io::Result<()> {
    #[cfg(windows)]
    {
        let owner = owner.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "clipboard window unavailable")
        })?;
        windows::write(text, owner)
    }
    #[cfg(not(windows))]
    {
        let _ = (text, owner);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard unavailable",
        ))
    }
}

pub fn read_clipboard(max_utf16_bytes: usize) -> io::Result<String> {
    #[cfg(windows)]
    {
        windows::read(max_utf16_bytes)
    }
    #[cfg(not(windows))]
    {
        let _ = max_utf16_bytes;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard unavailable",
        ))
    }
}

#[cfg(windows)]
mod windows {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{io, ptr};
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        },
    };

    const CF_UNICODETEXT: u32 = 13;

    struct OpenGuard;
    impl OpenGuard {
        fn new(owner: *mut std::ffi::c_void) -> io::Result<Self> {
            if unsafe { OpenClipboard(owner) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(Self)
            }
        }
    }
    impl Drop for OpenGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    pub(super) fn write(text: &str, owner: &impl HasWindowHandle) -> io::Result<()> {
        let handle = owner
            .window_handle()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Windows clipboard requires a Win32 window",
            ));
        };
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let bytes = wide.len().checked_mul(2).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "clipboard text too large")
        })?;
        let _open = OpenGuard::new(handle.hwnd.get() as *mut std::ffi::c_void)?;
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let data = unsafe { GlobalLock(handle) };
        if data.is_null() {
            unsafe {
                GlobalFree(handle);
            }
            return Err(io::Error::last_os_error());
        }
        unsafe {
            ptr::copy_nonoverlapping(wide.as_ptr(), data.cast::<u16>(), wide.len());
            GlobalUnlock(handle);
        }
        if unsafe { EmptyClipboard() } == 0 {
            unsafe {
                GlobalFree(handle);
            }
            return Err(io::Error::last_os_error());
        }
        if unsafe { SetClipboardData(CF_UNICODETEXT, handle) }.is_null() {
            unsafe {
                GlobalFree(handle);
            }
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn read(max_bytes: usize) -> io::Result<String> {
        let _open = OpenGuard::new(ptr::null_mut())?;
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if handle.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Unicode clipboard text unavailable",
            ));
        }
        let bytes = unsafe { GlobalSize(handle) };
        if bytes > max_bytes || bytes % 2 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "clipboard text exceeds limit",
            ));
        }
        let data = unsafe { GlobalLock(handle) };
        if data.is_null() {
            return Err(io::Error::last_os_error());
        }
        let units = unsafe { std::slice::from_raw_parts(data.cast::<u16>(), bytes / 2) };
        let end = units
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units.len());
        let text = String::from_utf16(&units[..end])
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid clipboard text"));
        unsafe {
            GlobalUnlock(handle);
        }
        text
    }
}
