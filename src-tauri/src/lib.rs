mod cookies;
mod download_state;
mod process_control;
mod progress;

use std::{
    collections::VecDeque,
    io::BufReader,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use tauri::Emitter;
use tauri::Manager;

use cookies::{has_nonempty_cookies_file, resolve_cookies_file_path};
use download_state::{ActiveDownload, BatchProgress, DownloadController, DownloadProgress};
use process_control::{cancel_process_tree, set_process_tree_suspended};
use progress::{emit_progress_from_reader, extract_username_from_url, sanitize_path_segment};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn resolve_binary_paths(app: &tauri::AppHandle) -> Option<(PathBuf, PathBuf)> {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("libs").join("yt-dlp.exe"));
        candidates.push(cwd.join("libs").join("yt-dlp").join("yt-dlp.exe"));
        candidates.push(cwd.join("libs").join("yt-dlp.exe"));
        candidates.push(cwd.join("yt-dlp.exe"));
        candidates.push(cwd.join("..").join("yt-dlp.exe"));
        candidates.push(
            cwd.join("..")
                .join("libs")
                .join("yt-dlp")
                .join("yt-dlp.exe"),
        );
        candidates.push(cwd.join("..").join("libs").join("yt-dlp.exe"));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("libs").join("yt-dlp.exe"));
        candidates.push(resource_dir.join("yt-dlp.exe"));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("libs").join("yt-dlp.exe"));
            candidates.push(exe_dir.join("yt-dlp.exe"));
        }
    }

    let yt_dlp = candidates.into_iter().find(|path| path.exists())?;
    let ffmpeg_dir = yt_dlp.parent()?.to_path_buf();

    if ffmpeg_dir.join("ffmpeg.exe").exists() {
        Some((yt_dlp, ffmpeg_dir))
    } else {
        None
    }
}

#[tauri::command]
fn has_cookies_file(app: tauri::AppHandle) -> bool {
    let cookies_path = resolve_cookies_file_path(&app);
    has_nonempty_cookies_file(&cookies_path)
}

#[tauri::command]
fn get_cookies_file_path(app: tauri::AppHandle) -> String {
    resolve_cookies_file_path(&app)
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
async fn download_video(
    app: tauri::AppHandle,
    controller: tauri::State<'_, Arc<DownloadController>>,
    url: String,
) -> Result<(), String> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        download_tiktok_blocking(app, controller, url, true)
    })
    .await
    .map_err(|e| format!("Download task failed: {e}"))?
}

#[tauri::command]
async fn download_profile(
    app: tauri::AppHandle,
    controller: tauri::State<'_, Arc<DownloadController>>,
    url: String,
) -> Result<(), String> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        download_tiktok_blocking(app, controller, url, false)
    })
    .await
    .map_err(|e| format!("Download task failed: {e}"))?
}

#[tauri::command]
fn toggle_pause_download(
    app: tauri::AppHandle,
    controller: tauri::State<'_, Arc<DownloadController>>,
) -> Result<bool, String> {
    let mut guard = controller
        .active
        .lock()
        .map_err(|_| "Could not lock download controller".to_string())?;
    let active = guard
        .as_mut()
        .ok_or_else(|| "No active download to pause".to_string())?;

    let should_pause = !active.paused;
    set_process_tree_suspended(active.pid, should_pause)?;
    active.paused = should_pause;

    let snapshot = active
        .state
        .lock()
        .ok()
        .map(|s| s.clone())
        .unwrap_or_default();
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            current_item: snapshot.current_item,
            current_title: snapshot.current_title,
            eta: snapshot.eta,
            percent: snapshot.percent,
            speed: snapshot.speed,
            status: if should_pause {
                "paused".to_string()
            } else {
                "downloading".to_string()
            },
            detail: if should_pause {
                "Paused".to_string()
            } else {
                "Resumed".to_string()
            },
            total_items: snapshot.total_items,
            username: active.username.clone(),
        },
    );

    Ok(should_pause)
}

#[tauri::command]
fn cancel_download(
    app: tauri::AppHandle,
    controller: tauri::State<'_, Arc<DownloadController>>,
) -> Result<(), String> {
    let guard = controller
        .active
        .lock()
        .map_err(|_| "Could not lock download controller".to_string())?;
    let active = guard
        .as_ref()
        .ok_or_else(|| "No active download to cancel".to_string())?;

    let snapshot = active
        .state
        .lock()
        .ok()
        .map(|s| s.clone())
        .unwrap_or_default();
    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            current_item: snapshot.current_item,
            current_title: snapshot.current_title,
            eta: snapshot.eta,
            percent: snapshot.percent,
            speed: snapshot.speed,
            status: "cancelled".to_string(),
            detail: "Cancelling active download".to_string(),
            total_items: snapshot.total_items,
            username: active.username.clone(),
        },
    );
    cancel_process_tree(active.pid)
}

fn download_tiktok_blocking(
    app: tauri::AppHandle,
    controller: Arc<DownloadController>,
    url: String,
    video_only: bool,
) -> Result<(), String> {
    let (yt_dlp, ffmpeg_dir) = resolve_binary_paths(&app).ok_or_else(|| {
        "Could not find yt-dlp.exe and ffmpeg.exe. In dev, put them in src-tauri/libs. In bundle, ensure they are included in resources/libs."
            .to_string()
    })?;
    let download_root = app
        .path()
        .download_dir()
        .map_err(|e| format!("Could not resolve Downloads folder: {e}"))?;

    let username = sanitize_path_segment(
        &extract_username_from_url(&url).unwrap_or_else(|| "unknown_user".to_string()),
    );
    let user_output_folder = download_root.join(&username);

    let output_path = user_output_folder.as_path();
    if !output_path.exists() {
        std::fs::create_dir_all(output_path).map_err(|e| {
            format!(
                "Could not create output folder '{}': {e}",
                output_path.to_string_lossy()
            )
        })?;
    }

    let output_template = "%(title).120B.%(ext)s";
    let cookies_file = resolve_cookies_file_path(&app);
    let logs: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    let mut cmd = Command::new(yt_dlp);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.arg("--no-continue")
        .arg("--cookies")
        .arg(cookies_file)
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("--ffmpeg-location")
        .arg(ffmpeg_dir)
        .arg("--encoding")
        .arg("utf-8")
        .arg("--windows-filenames")
        .arg("--newline")
        .arg("--sleep-interval")
        .arg("5")
        .arg("--max-sleep-interval")
        .arg("15")
        .arg("-P")
        .arg(output_path)
        .arg("-o")
        .arg(output_template)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1");

    if video_only {
        cmd.arg("--no-playlist");
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start yt-dlp.exe: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not read stdout from yt-dlp".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not read stderr from yt-dlp".to_string())?;

    let state: Arc<Mutex<BatchProgress>> = Arc::new(Mutex::new(BatchProgress::default()));
    let pid = child.id();
    {
        let mut guard = controller
            .active
            .lock()
            .map_err(|_| "Could not lock download controller".to_string())?;

        if guard.is_some() {
            return Err("Another download is already running".to_string());
        }

        *guard = Some(ActiveDownload {
            paused: false,
            pid,
            state: state.clone(),
            username: username.clone(),
        });
    }

    let out_handle = emit_progress_from_reader(
        BufReader::new(stdout),
        app.clone(),
        logs.clone(),
        state.clone(),
        username.clone(),
    );

    let err_handle = emit_progress_from_reader(
        BufReader::new(stderr),
        app.clone(),
        logs.clone(),
        state.clone(),
        username.clone(),
    );

    let status = child
        .wait()
        .map_err(|e| format!("yt-dlp failed while waiting for completion: {e}"))?;

    let _ = out_handle.join();
    let _ = err_handle.join();
    if let Ok(mut guard) = controller.active.lock() {
        if guard.as_ref().map(|active| active.pid) == Some(pid) {
            *guard = None;
        }
    }

    if status.success() {
        let final_state = state.lock().ok().map(|s| s.clone()).unwrap_or_default();
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                current_item: final_state.current_item.max(1),
                current_title: final_state.current_title,
                eta: if final_state.eta.is_empty() {
                    "0".to_string()
                } else {
                    final_state.eta
                },
                percent: 100.0,
                speed: "done".to_string(),
                status: "done".to_string(),
                detail: "Completed".to_string(),
                total_items: final_state.total_items.max(1),
                username,
            },
        );
        Ok(())
    } else {
        let was_cancelled = logs
            .lock()
            .ok()
            .map(|g| {
                g.iter()
                    .any(|line| line.contains("terminated") || line.contains("not found"))
            })
            .unwrap_or(false);
        let tail = logs
            .lock()
            .ok()
            .map(|g| g.iter().cloned().collect::<Vec<_>>().join("\n"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "No additional logs".to_string());

        if was_cancelled {
            Err("Download cancelled".to_string())
        } else {
            Err(format!(
                "yt-dlp exited with error: {status}\n--- yt-dlp log tail ---\n{tail}"
            ))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(DownloadController::default()))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            has_cookies_file,
            get_cookies_file_path,
            quit_app,
            download_video,
            download_profile,
            toggle_pause_download,
            cancel_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
