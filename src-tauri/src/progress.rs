use crate::download_state::{BatchProgress, DownloadProgress};
use std::{
    collections::VecDeque,
    io::BufRead,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::Emitter;

fn parse_progress_line(line: &str) -> Option<DownloadProgress> {
    let marker = "% of";
    let marker_idx = line.find(marker)?;

    let number_start = line[..marker_idx]
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let percent = line[number_start..marker_idx].trim().parse::<f64>().ok()?;

    let speed = line
        .split(" at ")
        .nth(1)
        .and_then(|part| part.split(" ETA ").next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let eta = line
        .split(" ETA ")
        .nth(1)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    Some(DownloadProgress {
        current_item: 0,
        current_title: "".to_string(),
        eta,
        percent,
        speed,
        status: "downloading".to_string(),
        detail: "".to_string(),
        total_items: 0,
        username: "".to_string(),
    })
}

fn parse_playlist_item_line(line: &str) -> Option<(u32, u32)> {
    let marker = "[download] Downloading item ";
    if !line.starts_with(marker) {
        return None;
    }

    let rest = &line[marker.len()..];
    let (current, total) = rest.split_once(" of ")?;
    let current = current.trim().parse::<u32>().ok()?;
    let total = total.trim().parse::<u32>().ok()?;

    Some((current, total))
}

fn parse_destination_title(line: &str) -> Option<String> {
    let marker = "[download] Destination: ";
    let value = line.strip_prefix(marker)?.trim();
    let name = Path::new(value)
        .file_stem()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| value.to_string());
    Some(name)
}

pub fn extract_username_from_url(url: &str) -> Option<String> {
    let marker = "/@";
    let start = url.find(marker)? + marker.len();
    let rest = &url[start..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let username = rest[..end].trim();
    if username.is_empty() {
        None
    } else {
        Some(username.to_string())
    }
}

pub fn sanitize_path_segment(name: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut out = name
        .chars()
        .map(|c| if invalid.contains(&c) { '_' } else { c })
        .collect::<String>();
    out = out.trim().trim_end_matches('.').to_string();
    if out.is_empty() {
        "unknown_user".to_string()
    } else {
        out
    }
}

pub fn build_progress_from_line(
    line: &str,
    state: &Arc<Mutex<BatchProgress>>,
    username: &str,
) -> Option<DownloadProgress> {
    if let Some((current, total)) = parse_playlist_item_line(line) {
        if let Ok(mut guard) = state.lock() {
            guard.current_item = current;
            guard.total_items = total;
        }

        return Some(DownloadProgress {
            current_item: current,
            current_title: "".to_string(),
            eta: "-".to_string(),
            percent: if total > 0 {
                ((current.saturating_sub(1)) as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            speed: "-".to_string(),
            status: "downloading".to_string(),
            detail: format!("Downloading video {current}/{total}"),
            total_items: total,
            username: username.to_string(),
        });
    }

    if let Some(title) = parse_destination_title(line) {
        if let Ok(mut guard) = state.lock() {
            guard.current_title = title.clone();
            if guard.current_item == 0 {
                guard.current_item = 1;
            }
            if guard.total_items == 0 {
                guard.total_items = 1;
            }

            return Some(DownloadProgress {
                current_item: guard.current_item,
                current_title: title,
                eta: "-".to_string(),
                percent: if guard.total_items > 0 {
                    ((guard.current_item.saturating_sub(1)) as f64 / guard.total_items as f64)
                        * 100.0
                } else {
                    0.0
                },
                speed: "-".to_string(),
                status: "downloading".to_string(),
                detail: format!(
                    "Downloading video {}/{}",
                    guard.current_item, guard.total_items
                ),
                total_items: guard.total_items,
                username: username.to_string(),
            });
        }
    }

    let mut progress = parse_progress_line(line)?;
    if let Ok(guard) = state.lock() {
        progress.current_item = guard.current_item;
        progress.current_title = guard.current_title.clone();
        progress.total_items = guard.total_items;
        progress.username = username.to_string();
        progress.detail = if guard.total_items > 1 {
            format!(
                "Downloading video {}/{}",
                guard.current_item, guard.total_items
            )
        } else {
            "Downloading video".to_string()
        };

        if guard.total_items > 1 && guard.current_item > 0 {
            let overall = ((guard.current_item.saturating_sub(1)) as f64
                + (progress.percent / 100.0))
                / guard.total_items as f64
                * 100.0;
            progress.percent = overall.min(100.0);
        }
    }

    if let Ok(mut guard) = state.lock() {
        guard.eta = progress.eta.clone();
        guard.percent = progress.percent;
        guard.speed = progress.speed.clone();
    }

    Some(progress)
}

pub fn push_log(logs: &Arc<Mutex<VecDeque<String>>>, line: String) {
    if let Ok(mut guard) = logs.lock() {
        if guard.len() >= 40 {
            guard.pop_front();
        }
        guard.push_back(line);
    }
}

pub fn emit_progress_from_reader<R: BufRead + Send + 'static>(
    reader: R,
    app: tauri::AppHandle,
    logs: Arc<Mutex<VecDeque<String>>>,
    state: Arc<Mutex<BatchProgress>>,
    username: String,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in reader.lines().map_while(Result::ok) {
            push_log(&logs, line.clone());
            if let Some(progress) = build_progress_from_line(&line, &state, &username) {
                let _ = app.emit("download-progress", progress);
            }
        }
    })
}