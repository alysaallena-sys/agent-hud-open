//! macOS seatbelt. Applied before any opted-in file is read.
//! `sandbox_init` is the process boundary. A failure refuses the read.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> i32;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

pub fn confine(roots: &[PathBuf]) -> Result<bool, String> {
    let mut profile = String::from(
        "(version 1)\n\
(deny default)\n\
(deny network*)\n\
(deny network-inbound)\n\
(deny network-outbound)\n\
(deny network-bind)\n\
(deny file-write*)\n\
(allow file-read* (subpath \"/usr/lib\") (subpath \"/System\") (subpath \"/usr/share\") (subpath \"/Library\") (subpath \"/private/var/db\"))\n\
(allow signal (target self))\n\
(allow sysctl-read)\n\
(allow mach-lookup\n\
    (global-name \"com.apple.system.logger\")\n\
    (global-name \"com.apple.system.notification_center\")\n\
    (global-name \"com.apple.CoreServices.coreservicesd\")\n\
    (global-name \"com.apple.distributed_notifications@Uv3\"))\n",
    );
    if let Ok(executable) = std::env::current_exe() {
        allow_read(&mut profile, &executable, true)?;
        if let Some(directory) = executable.parent() {
            allow_read(&mut profile, directory, false)?;
        }
    }
    for root in roots {
        allow_read(&mut profile, root, false)?;
    }
    let profile = CString::new(profile).map_err(|_| "sandbox profile was rejected".to_owned())?;
    let mut error = std::ptr::null_mut();
    let code = unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) };
    if code != 0 {
        let detail = if error.is_null() {
            String::new()
        } else {
            let text = unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned();
            unsafe { sandbox_free_error(error) };
            text
        };
        return Err(format!("sandbox_init failed ({code}) {detail}"));
    }
    Ok(true)
}

pub fn self_test(inside: &Path, outside: &Path) -> Result<(), String> {
    confine(&[inside.to_path_buf()])?;
    let outside_denied = std::fs::read(outside).is_err();
    let write_denied = std::fs::write(inside.join("sandbox-write-probe"), b"no").is_err();
    let inside_ok = std::fs::read(inside.join("note.txt")).is_ok();
    println!("sandbox_applied=true");
    println!(
        "outside_read={}",
        if outside_denied { "denied" } else { "allowed" }
    );
    println!("write={}", if write_denied { "denied" } else { "allowed" });
    println!("inside_read={}", if inside_ok { "ok" } else { "denied" });
    if outside_denied && write_denied && inside_ok {
        Ok(())
    } else {
        Err("sandbox self-test failed".to_owned())
    }
}

fn allow_read(profile: &mut String, path: &Path, literal: bool) -> Result<(), String> {
    let text = path.to_string_lossy();
    if text.bytes().any(|byte| {
        !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b' ' | b'+'))
    }) {
        return Err("refusing a path the sandbox profile cannot quote".to_owned());
    }
    if literal {
        profile.push_str(&format!("(allow file-read* (literal \"{text}\"))\n"));
    } else {
        profile.push_str(&format!("(allow file-read* (subpath \"{text}\"))\n"));
    }
    Ok(())
}
