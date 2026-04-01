use std::{fs, path::PathBuf};
use tauri::Manager;

pub fn resolve_cookies_file_path(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let resource_cookies = resource_dir.join("cookies.txt");
        if resource_cookies.exists() {
            return resource_cookies;
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_cookies = exe_dir.join("cookies.txt");
            if exe_cookies.exists() {
                return exe_cookies;
            }
        }
    }

    PathBuf::from("cookies.txt")
}

pub fn has_nonempty_cookies_file(path: &std::path::Path) -> bool {
    match fs::read_to_string(path) {
        Ok(contents) => !contents.trim().is_empty(),
        Err(_) => false,
    }
}
