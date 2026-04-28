//! Session information gathering.

use std::env;

use super::models::{SessionInfo, SessionType};

/// Get current session information.
pub fn get_info() -> SessionInfo {
    let session_type = detect_session_type();
    let user = get_user();
    let seat = get_seat();
    let home = get_home();
    let shell = get_shell();
    let terminal = get_terminal();
    let uptime = get_uptime();

    SessionInfo {
        session_type,
        user,
        seat,
        home,
        shell,
        terminal,
        uptime_seconds: uptime,
    }
}

/// Detect session type (Wayland/X11/TTY).
fn detect_session_type() -> SessionType {
    // Check XDG_SESSION_TYPE first
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "wayland" => return SessionType::Wayland,
            "x11" => return SessionType::X11,
            "tty" => return SessionType::Tty,
            _ => {}
        }
    }

    // Fallback: check for Wayland-specific variables
    if env::var("WAYLAND_DISPLAY").is_ok() || env::var("WAYLAND_SOCKET").is_ok() {
        return SessionType::Wayland;
    }

    // Check for X11
    if env::var("DISPLAY").is_ok() {
        return SessionType::X11;
    }

    SessionType::Unknown
}

/// Get current user.
fn get_user() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| {
            let uid = unsafe { libc::getuid() };
            get_username_from_passwd(uid)
        })
}

/// Get home directory.
fn get_home() -> String {
    env::var("HOME").unwrap_or_else(|_| {
        // Fallback to passwd lookup
        let uid = unsafe { libc::getuid() };
        get_home_from_passwd(uid)
    })
}

/// Get seat from environment.
fn get_seat() -> Option<String> {
    env::var("XDG_SEAT").ok()
}

/// Fallback: look up user by UID.
fn get_username_from_passwd(uid: libc::uid_t) -> String {
    #[cfg(not(test))]
    {
        use std::ffi::CStr;
        use std::mem::MaybeUninit;

        let mut passwd = MaybeUninit::<libc::passwd>::uninit();
        let mut buf = vec![0u8; 1024];

        let result = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut std::ptr::null_mut(),
            )
        };

        if result == 0 {
            let pw = unsafe { passwd.assume_init() };
            if !pw.pw_name.is_null() {
                return unsafe { CStr::from_ptr(pw.pw_name) }
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }

    String::new()
}

/// Fallback: look up home directory by UID.
fn get_home_from_passwd(uid: libc::uid_t) -> String {
    #[cfg(not(test))]
    {
        use std::ffi::CStr;
        use std::mem::MaybeUninit;

        let mut passwd = MaybeUninit::<libc::passwd>::uninit();
        let mut buf = vec![0u8; 1024];

        let result = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut std::ptr::null_mut(),
            )
        };

        if result == 0 {
            let pw = unsafe { passwd.assume_init() };
            if !pw.pw_dir.is_null() {
                return unsafe { CStr::from_ptr(pw.pw_dir) }
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }

    String::new()
}

/// Get shell (e.g., "zsh", "bash").
fn get_shell() -> Option<String> {
    env::var("SHELL").ok()
}

/// Get terminal emulator.
fn get_terminal() -> Option<String> {
    // Try various terminal indicators
    if let Ok(v) = env::var("TERM_PROGRAM") {
        return Some(v);
    }
    if env::var("ALACRITTY_SOCKET").is_ok() {
        return Some("alacritty".to_string());
    }
    if env::var("KITTY_WINDOW_ID").is_ok() {
        return Some("kitty".to_string());
    }
    if let Ok(v) = env::var("WAYLAND_TERMINAL") {
        return Some(v);
    }
    // Fallback: check TERM
    env::var("TERM").ok()
}

/// Get system uptime in seconds.
fn get_uptime() -> Option<u64> {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .map(|v| v as u64)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_user() {
        let user = get_user();
        // User should not be empty in a real session
        assert!(!user.is_empty());
    }

    #[test]
    fn test_get_home() {
        let home = get_home();
        assert!(!home.is_empty());
    }
}
