use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL,
    FILE_GENERIC_WRITE, FILE_SHARE_NONE, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use windows::Win32::System::Threading::CreateMutexW;

pub const MUTEX_NAME: &str = "Local\\AuraWaveDesktopMutex";
pub const PIPE_NAME: &str = "\\\\.\\pipe\\AuraWaveDesktopPipe";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct SingleInstanceGuard {
    mutex_handle: HANDLE,
}

impl SingleInstanceGuard {
    pub fn acquire_or_notify(pending_url: Option<&str>) -> Option<Self> {
        let wide_mutex = to_wide(MUTEX_NAME);
        let handle = unsafe { CreateMutexW(None, true, PCWSTR(wide_mutex.as_ptr())) }.ok()?;
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

        if already_exists {
            let msg = pending_url.unwrap_or("FOCUS");
            send_to_existing_instance(msg);
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }

        Some(Self {
            mutex_handle: handle,
        })
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.mutex_handle);
        }
    }
}

fn send_to_existing_instance(message: &str) {
    let wide_pipe = to_wide(PIPE_NAME);
    let line = format!("{}\n", message);
    let bytes = line.as_bytes();

    for _ in 0..8 {
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_pipe.as_ptr()),
                FILE_GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };

        if let Ok(h) = handle {
            if h != INVALID_HANDLE_VALUE {
                let mut written = 0u32;
                unsafe {
                    let _ = WriteFile(h, Some(bytes), Some(&mut written), None);
                    let _ = FlushFileBuffers(h);
                    let _ = CloseHandle(h);
                }
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

pub fn spawn_pipe_server<F>(stop_flag: Arc<AtomicBool>, on_message: F)
where
    F: Fn(String) + Send + 'static,
{
    let wide_pipe = to_wide(PIPE_NAME);
    std::thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            let pipe = unsafe {
                CreateNamedPipeW(
                    PCWSTR(wide_pipe.as_ptr()),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    1,
                    1024,
                    4096,
                    0,
                    None,
                )
            };

            if pipe == INVALID_HANDLE_VALUE {
                std::thread::sleep(std::time::Duration::from_millis(250));
                continue;
            }

            let connected = unsafe {
                ConnectNamedPipe(pipe, None).is_ok() || GetLastError() == ERROR_PIPE_CONNECTED
            };

            if connected && !stop_flag.load(Ordering::Relaxed) {
                let mut buf = [0u8; 2048];
                let mut read = 0u32;
                let ok = unsafe { ReadFile(pipe, Some(&mut buf), Some(&mut read), None).is_ok() };
                if ok && read > 0 {
                    let text = String::from_utf8_lossy(&buf[..read as usize]).trim().to_string();
                    if !text.is_empty() {
                        on_message(text);
                    }
                }
            }

            unsafe {
                let _ = DisconnectNamedPipe(pipe);
                let _ = CloseHandle(pipe);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_identifiers_are_distinct_for_aurawave() {
        assert_eq!(MUTEX_NAME, "Local\\AuraWaveDesktopMutex");
        assert_eq!(PIPE_NAME, "\\\\.\\pipe\\AuraWaveDesktopPipe");
        assert!(!MUTEX_NAME.contains("SoundCloudDesktopRust"));
        assert!(!PIPE_NAME.contains("SoundCloudDesktopRust"));
    }
}
