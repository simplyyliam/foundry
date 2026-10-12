//! The OS window corner radius, so the frontend can match it.
//!
//! No OS call returns a custom corner radius, so this detects per environment:
//! - Windows: the Windhawk mod `custom-corner-radius` (read-only registry reads).
//! - macOS 26 and later: 26.
//! - Anything else, or any read error: `None`.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WindowRadius {
    /// Corner radius in pixels, or null when it can't be detected.
    pub radius: Option<u32>,
}

/// The largest radius the Windhawk setting may hold before it's treated as bogus.
const MAX_RADIUS: u32 = 64;
/// The corner radius macOS 26 (Tahoe) gives standard windows.
const MACOS_RADIUS: u32 = 26;

/// A registry value as far as detection cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegValue {
    Dword(u32),
    /// Present, but not a REG_DWORD (a string, binary, ...).
    NotDword,
}

/// Interprets the Windhawk mod's `Disabled` value (on the mod key) and its `radius` setting.
/// `None` means the value (or its key) is missing. A missing `Disabled` means enabled.
pub fn windhawk_radius(disabled: Option<RegValue>, radius: Option<RegValue>) -> Option<u32> {
    match disabled {
        None | Some(RegValue::Dword(0)) => {}
        Some(_) => return None,
    }
    match radius? {
        RegValue::Dword(px @ 1..=MAX_RADIUS) => Some(px),
        _ => None,
    }
}

/// The radius for a macOS version string as `sw_vers -productVersion` prints it ("26.0").
pub fn macos_radius(version: &str) -> Option<u32> {
    let major: u32 = version.trim().split('.').next()?.parse().ok()?;
    (major >= 26).then_some(MACOS_RADIUS)
}

/// Detects the radius for the running OS. Never fails and never writes.
pub fn detect() -> WindowRadius {
    WindowRadius {
        radius: detect_radius(),
    }
}

#[cfg(windows)]
fn detect_radius() -> Option<u32> {
    const MOD_KEY: &str = r"SOFTWARE\Windhawk\Engine\Mods\custom-corner-radius";
    // A failed read (other than "missing") gives up rather than guessing.
    let disabled = registry::read_hklm(MOD_KEY, "Disabled")?;
    let radius = registry::read_hklm(&format!(r"{MOD_KEY}\Settings"), "radius")?;
    windhawk_radius(disabled, radius)
}

#[cfg(target_os = "macos")]
fn detect_radius() -> Option<u32> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    macos_radius(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect_radius() -> Option<u32> {
    None
}

#[cfg(windows)]
mod registry {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, REG_DWORD, REG_VALUE_TYPE, RRF_RT_ANY,
        RRF_SUBKEY_WOW6464KEY,
    };

    use super::RegValue;

    /// Reads one value under HKLM (64-bit view). `Some(None)` when the key or value is
    /// missing, `NotDword` for a value of any other type, `None` when the read fails.
    pub fn read_hklm(subkey: &str, name: &str) -> Option<Option<RegValue>> {
        let mut kind = REG_VALUE_TYPE::default();
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: `data` and `size` describe a valid 4-byte buffer that outlives the call.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                &HSTRING::from(subkey),
                &HSTRING::from(name),
                RRF_RT_ANY | RRF_SUBKEY_WOW6464KEY,
                Some(&mut kind),
                Some((&mut data as *mut u32).cast()),
                Some(&mut size),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Some(None);
        }
        if status == ERROR_SUCCESS && kind == REG_DWORD && size == 4 {
            return Some(Some(RegValue::Dword(data)));
        }
        // ERROR_MORE_DATA: present, but bigger than a DWORD.
        if status == ERROR_SUCCESS || status == ERROR_MORE_DATA {
            return Some(Some(RegValue::NotDword));
        }
        None
    }
}
