use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config, Event, EventKind};

// --- State Management ---
pub struct AppState {
    watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    processed: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Serialize)]
struct FileProcessedPayload {
    file_name: String,
    directory: String,
    status: String,
}

// --- Helper: Wait for OS to finish writing the file ---
fn wait_for_file_ready(path: &Path) -> Result<(), String> {
    let mut last_size = 0;
    // Check up to 50 times, 100ms apart = 5 seconds max wait
    for _ in 0..50 {
        if let Ok(metadata) = std::fs::metadata(path) {
            let size = metadata.len();
            // If size is > 0 and hasn't changed since the last check, it's done writing
            if size > 0 && size == last_size {
                return Ok(());
            }
            last_size = size;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Timeout: File did not finish writing".to_string())
}

// --- Tauri Commands ---

#[tauri::command]
async fn add_directory(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    let mut watchers_lock = state.watchers.lock().unwrap();

    if watchers_lock.contains_key(&path) {
        return Ok(());
    }

    let app_clone = app.clone();
    let processed_clone = state.processed.clone();
    let dir_path = path.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for file_path in event.paths {
                        if !file_path.is_file() { continue; }

                        let path_str = file_path.to_string_lossy().to_string();

                        // 1. Prevent duplicate processing (Windows fires multiple events per file)
                        {
                            let mut processed = processed_clone.lock().unwrap();
                            if processed.contains(&path_str) { continue; }
                            processed.insert(path_str.clone());
                        }

                        let ext = file_path.extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        let supported = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "pdf", "mp4", "mov", "avi", "mkv", "webm"];

                        if supported.contains(&ext.as_str()) {
                            let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();
                            let app_emit = app_clone.clone();
                            let dir_emit = dir_path.clone();
                            let file_path_clone = file_path.clone();

                            // 2. Spawn a thread so we don't block the file watcher event loop
                            std::thread::spawn(move || {
                                // Wait dynamically for the OS to finish writing the file
                                if let Err(e) = wait_for_file_ready(&file_path_clone) {
                                    let _ = app_emit.emit("file_processed", FileProcessedPayload {
                                        file_name: file_name.clone(),
                                        directory: dir_emit.clone(),
                                        status: format!("error: {}", e),
                                    });
                                    return;
                                }

                                // 3. Process the file
                                let ext_str = file_path_clone.extension().unwrap().to_str().unwrap().to_lowercase();
                                let status = match ext_str.as_str() {
                                    "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => process_image(&file_path_clone),
                                    "pdf" => process_pdf(&file_path_clone),
                                    _ => process_video(&file_path_clone),
                                };

                                // 4. Emit event to frontend
                                let _ = app_emit.emit("file_processed", FileProcessedPayload {
                                    file_name,
                                    directory: dir_emit,
                                    status: if status.is_ok() { "success".into() } else { format!("error: {}", status.err().unwrap_or_default()) },
                                });
                            });
                        }
                    }
                }
            }
        },
        Config::default(),
    ).map_err(|e| e.to_string())?;

    watcher.watch(Path::new(&path), RecursiveMode::Recursive).map_err(|e| e.to_string())?;
    watchers_lock.insert(path, watcher);
    Ok(())
}

#[tauri::command]
async fn remove_directory(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let mut watchers = state.watchers.lock().unwrap();
    watchers.remove(&path);
    Ok(())
}

#[tauri::command]
async fn get_directories(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.watchers.lock().unwrap().keys().cloned().collect())
}

// --- File Processors ---

fn process_image(path: &PathBuf) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    img.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn process_pdf(path: &PathBuf) -> Result<(), String> {
    use lopdf::Document;

    let mut doc = Document::load(path).map_err(|e| e.to_string())?;
    doc.trailer.remove(b"Info");

    if let Ok(lopdf::Object::Reference(catalog_id)) = doc.trailer.get(b"Root") {
        if let Ok(lopdf::Object::Dictionary(ref mut catalog)) = doc.get_object_mut(*catalog_id) {
            catalog.remove(b"Metadata");
        }
    }

    doc.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn process_video(path: &PathBuf) -> Result<(), String> {
    let path_str = path.to_str().ok_or("Invalid path")?;
    let temp_path = format!("{}.poisto_tmp", path_str);

    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y", "-i", path_str,
            "-map_metadata", "-1",
            "-c:v", "copy", "-c:a", "copy",
            &temp_path
        ])
        .output()
        .map_err(|e| format!("ffmpeg execution error: {}", e))?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    std::fs::rename(&temp_path, path_str).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        e.to_string()
    })?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            watchers: Arc::new(Mutex::new(HashMap::new())),
            processed: Arc::new(Mutex::new(HashSet::new())),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            add_directory,
            remove_directory,
            get_directories
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
