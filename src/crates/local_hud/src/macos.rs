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
        allow_literal(&mut profile, &executable)?;
        if let Some(directory) = executable.parent() {
            allow_tree(&mut profile, directory)?;
        }
    }
    for root in roots {
        // `/var` on macOS is a symlink to `/private/var`. A subpath filter on
        // the spelling the caller used does not cover the resolved path, so
        // the seatbelt rejected opted-in temporary directories.
        allow_tree(&mut profile, root)?;
        if let Ok(canonical) = root.canonicalize() {
            if canonical != *root {
                allow_tree(&mut profile, &canonical)?;
            }
        }
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

fn allow_literal(profile: &mut String, path: &Path) -> Result<(), String> {
    let text = quoted(path)?;
    profile.push_str(&format!("(allow file-read* (literal \"{text}\"))\n"));
    Ok(())
}

fn allow_tree(profile: &mut String, path: &Path) -> Result<(), String> {
    let mut ancestor = PathBuf::new();
    for component in path.components() {
        ancestor.push(component);
        if ancestor == path {
            break;
        }
        let text = quoted(&ancestor)?;
        profile.push_str(&format!(
            "(allow file-read-metadata (literal \"{text}\"))\n"
        ));
    }
    let text = quoted(path)?;
    profile.push_str(&format!("(allow file-read* (subpath \"{text}\"))\n"));
    Ok(())
}

fn quoted(path: &Path) -> Result<String, String> {
    let text = path.to_string_lossy();
    if text.bytes().any(|byte| {
        !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b' ' | b'+'))
    }) {
        return Err("refusing a path the sandbox profile cannot quote".to_owned());
    }
    Ok(text.into_owned())
}
