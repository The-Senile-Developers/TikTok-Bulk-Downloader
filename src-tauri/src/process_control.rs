use std::{mem::size_of, process::Command};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, Thread32First, Thread32Next,
            PROCESSENTRY32W, TH32CS_SNAPPROCESS, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        Threading::{OpenThread, ResumeThread, SuspendThread, THREAD_SUSPEND_RESUME},
    },
};

#[cfg(windows)]
fn collect_process_tree(root_pid: u32) -> Result<Vec<u32>, String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("Could not snapshot running processes".to_string());
    }

    let mut processes = Vec::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

    let mut success = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while success {
        processes.push((entry.th32ProcessID, entry.th32ParentProcessID));
        success = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }

    unsafe {
        CloseHandle(snapshot);
    }

    let mut result = vec![root_pid];
    let mut cursor = 0;
    while cursor < result.len() {
        let parent_pid = result[cursor];
        for (pid, parent) in &processes {
            if *parent == parent_pid && !result.contains(pid) {
                result.push(*pid);
            }
        }
        cursor += 1;
    }

    Ok(result)
}

#[cfg(windows)]
pub fn set_process_tree_suspended(root_pid: u32, suspend: bool) -> Result<(), String> {
    let pids = collect_process_tree(root_pid)?;
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("Could not snapshot running threads".to_string());
    }

    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<THREADENTRY32>() as u32;
    let mut success = unsafe { Thread32First(snapshot, &mut entry) } != 0;

    while success {
        if pids.contains(&entry.th32OwnerProcessID) {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                unsafe {
                    if suspend {
                        SuspendThread(thread);
                    } else {
                        ResumeThread(thread);
                    }
                    CloseHandle(thread);
                }
            }
        }

        success = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }

    unsafe {
        CloseHandle(snapshot);
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn set_process_tree_suspended(_root_pid: u32, _suspend: bool) -> Result<(), String> {
    Err("Pause/resume is currently only implemented on Windows".to_string())
}

#[cfg(windows)]
pub fn cancel_process_tree(root_pid: u32) -> Result<(), String> {
    let status = Command::new("taskkill")
        .args(["/PID", &root_pid.to_string(), "/T", "/F"])
        .status()
        .map_err(|e| format!("Could not cancel active download: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("taskkill exited with status: {status}"))
    }
}

#[cfg(not(windows))]
pub fn cancel_process_tree(_root_pid: u32) -> Result<(), String> {
    Err("Cancel is currently only implemented on Windows".to_string())
}