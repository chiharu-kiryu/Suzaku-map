//! Configuration-only data management. No input history or model weights are collected.

pub mod backup;
pub mod files;
pub mod paths;

#[cfg(target_os = "linux")]
pub fn open_directory(path: &std::path::Path) -> Result<(), String> {
    use std::{os::unix::ffi::OsStrExt, process::Command, time::Duration};
    if !path.is_absolute() {
        return Err("目录路径必须是绝对路径".into());
    }
    files::private_directory(path).map_err(|error| error.to_string())?;
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            uri.push(*byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    // Ask the desktop's service to open a folder; never run/kill the file manager itself.
    let output = crate::platform::linux_command::run(
        Command::new("gdbus").args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
            "--method",
            "org.freedesktop.FileManager1.ShowFolders",
            &format!("['{uri}']"),
            "",
        ]),
        Duration::from_secs(3),
    )
    .map_err(|error| format!("打开目录失败：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "文件管理器服务不可用，请手动打开：{}",
            path.display()
        ))
    }
}
