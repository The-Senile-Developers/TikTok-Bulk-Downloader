use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub current_item: u32,
    pub current_title: String,
    pub eta: String,
    pub percent: f64,
    pub speed: String,
    pub status: String,
    pub detail: String,
    pub total_items: u32,
    pub username: String,
}

#[derive(Clone, Default)]
pub struct BatchProgress {
    pub current_item: u32,
    pub current_title: String,
    pub eta: String,
    pub percent: f64,
    pub speed: String,
    pub total_items: u32,
}

pub struct ActiveDownload {
    pub paused: bool,
    pub pid: u32,
    pub state: Arc<Mutex<BatchProgress>>,
    pub username: String,
}

#[derive(Default)]
pub struct DownloadController {
    pub active: Mutex<Option<ActiveDownload>>,
}