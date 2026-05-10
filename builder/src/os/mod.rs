#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as o;
#[cfg(target_os = "macos")]
use macos as o;
#[cfg(target_os = "windows")]
use windows as o;

pub struct DeviceInfo {
    pub loc: String,
    pub name: String,
}

pub fn devices() -> Result<Vec<DeviceInfo>, String> {
    o::devices()
}

pub fn is_elevated() -> bool {
    is_sudo::check() == is_sudo::RunningAs::Root
}
