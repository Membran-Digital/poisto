use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

// Helper to get the hidden Alternate Data Stream path (e.g., "image.jpg:poisto_cleansed")
fn get_ads_path(path: &Path) -> OsString {
    let mut ads_path = OsString::from(path.as_os_str());
    ads_path.push(":poisto_cleansed");
    ads_path
}

fn is_cleansed(path: &Path) -> bool {
    // 1. PDF internal check (Cross-platform)
    if path.extension().and_then(|s| s.to_str()) == Some("pdf") {
        if let Ok(doc) = lopdf::Document::load(path) {
            if let Ok(lopdf::Object::Dictionary(info)) = doc.trailer.get(b"Info") {
                if let Ok(lopdf::Object::String(val, _)) = info.get(b"PoistoCleansed") {
                    if val == b"true" {
                        return true;
                    }
                }
            }
        }
    }

    let path_str = path.to_str().unwrap_or("");

    // 2. Windows: Native Alternate Data Streams (ADS)
    #[cfg(target_os = "windows")]
    {
        let mut ads_path = OsString::from(path.as_os_str());
        ads_path.push(":poisto_cleansed");
        if let Ok(mut file) = std::fs::File::open(&ads_path) {
            use std::io::Read;
            let mut buf = [0u8; 4];
            if file.read_exact(&mut buf).is_ok() && &buf == b"true" {
                return true;
            }
        }
    }

    // 3. macOS: Native xattr command
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("xattr")
            .args(["-p", "poisto_cleansed", path_str])
            .output()
        {
            if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
                return true;
            }
        }
    }

    // 4. Linux: Native getfattr command
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("getfattr")
            .args(["-n", "user.poisto_cleansed", "--only-values", path_str])
            .output()
        {
            if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
                return true;
            }
        }
    }

    false
}

fn mark_cleansed(path: &Path) {
    let path_str = path.to_str().unwrap_or("");

    // 1. Windows: Native Alternate Data Streams (ADS)
    #[cfg(target_os = "windows")]
    {
        let mut ads_path = OsString::from(path.as_os_str());
        ads_path.push(":poisto_cleansed");
        let _ = std::fs::write(&ads_path, b"true");
    }

    // 2. macOS: Native xattr command
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .args(["-w", "poisto_cleansed", "true", path_str])
            .output();
    }

    // 3. Linux: Native setfattr command
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("setfattr")
            .args(["-n", "user.poisto_cleansed", "-v", "true", path_str])
            .output();
    }

    // 4. PDF internal write (Cross-platform)
    if path.extension().and_then(|s| s.to_str()) == Some("pdf") {
        if let Ok(mut doc) = lopdf::Document::load(path) {
            let mut info_dict = lopdf::Dictionary::new();
            info_dict.set(
                "PoistoCleansed",
                lopdf::Object::String(b"true".to_vec(), lopdf::StringFormat::Literal),
            );
            let _ = doc
                .trailer
                .set("Info", lopdf::Object::Dictionary(info_dict));

            if let Ok(lopdf::Object::Reference(catalog_id)) = doc.trailer.get(b"Root") {
                if let Ok(lopdf::Object::Dictionary(ref mut catalog)) =
                    doc.get_object_mut(*catalog_id)
                {
                    catalog.remove(b"Metadata");
                }
            }
            let _ = doc.save(path);
        }
    }
}

// --- State Management ---
pub struct AppState {
    watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    active_processing: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Serialize)]
struct FileProcessedPayload {
    file_name: String,
    directory: String,
    status: String,
}

// --- Helpers: Pure, Lean File Flagging ---

fn get_flag_path(path: &Path) -> PathBuf {
    // Creates a 0-byte sidecar file: "image.jpg" -> "image.jpg.poisto"
    // This is universally supported, 0 bytes, and survives USB/Cloud transfers.
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    path.with_extension(format!("{}.poisto", ext))
}

fn wait_for_file_ready(path: &Path) -> Result<(), String> {
    let mut last_size = 0;
    for _ in 0..50 {
        if let Ok(metadata) = std::fs::metadata(path) {
            let size = metadata.len();
            if size > 0 && size == last_size {
                return Ok(());
            }
            last_size = size;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Timeout: File did not finish writing".to_string())
}

// --- Initial Bulk Scan ---
fn scan_and_clean_existing_files(app_handle: &AppHandle, dir_path: String) {
    let extensions = [
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "pdf", "mp4", "mov", "avi", "mkv", "webm",
    ];
    let app_clone = app_handle.clone();
    let dir_clone = dir_path.clone();

    std::thread::spawn(move || {
        for entry in WalkDir::new(&dir_clone).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !extensions.contains(&ext.as_str()) {
                continue;
            }

            // CHECK FLAG BEFORE DOING ANY WORK
            if is_cleansed(&path) {
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let _ = app_clone.emit(
                    "file_processed",
                    FileProcessedPayload {
                        file_name,
                        directory: dir_clone.clone(),
                        status: "skipped".into(),
                    },
                );
                continue;
            }

            let file_name = path.file_name().unwrap().to_string_lossy().to_string();

            // Process the file immediately (No "cleaning" status emitted)
            let status = match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => process_image(&path),
                "pdf" => process_pdf(&path),
                _ => process_video(&path),
            };

            let is_success = status.is_ok();
            let status_msg = if is_success {
                "success".into()
            } else {
                format!("error: {}", status.err().unwrap_or_default())
            };

            let _ = app_clone.emit(
                "file_processed",
                FileProcessedPayload {
                    file_name,
                    directory: dir_clone.clone(),
                    status: status_msg,
                },
            );

            if is_success {
                mark_cleansed(&path);
            }
        }
    });
}

// --- Live Watcher Setup ---
fn start_watcher(
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut watchers_lock = state.watchers.lock().unwrap();
    if watchers_lock.contains_key(&path) {
        return Ok(());
    }

    let app_for_closure = app_handle.clone();
    let active_for_closure = state.active_processing.clone();
    let dir_for_closure = path.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for file_path in event.paths {
                        if !file_path.is_file() {
                            continue;
                        }

                        let path_str = file_path.to_string_lossy().to_string();

                        {
                            let mut active = active_for_closure.lock().unwrap();
                            if active.contains(&path_str) {
                                continue;
                            }
                            active.insert(path_str.clone());
                        }

                        let ext = file_path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let supported = [
                            "jpg", "jpeg", "png", "gif", "bmp", "webp", "pdf", "mp4", "mov", "avi",
                            "mkv", "webm",
                        ];

                        if supported.contains(&ext.as_str()) {
                            let file_name =
                                file_path.file_name().unwrap().to_string_lossy().to_string();
                            let app_emit = app_for_closure.clone();
                            let dir_emit = dir_for_closure.clone();
                            let file_path_clone = file_path.clone();
                            let active_thread = active_for_closure.clone();

                            std::thread::spawn(move || {
                                let _guard = scopeguard::guard((), |_| {
                                    active_thread.lock().unwrap().remove(&path_str);
                                });

                                if let Err(e) = wait_for_file_ready(&file_path_clone) {
                                    let _ = app_emit.emit(
                                        "file_processed",
                                        FileProcessedPayload {
                                            file_name: file_name.clone(),
                                            directory: dir_emit.clone(),
                                            status: format!("error: {}", e),
                                        },
                                    );
                                    return;
                                }

                                // CHECK FLAG BEFORE DOING ANY WORK
                                if is_cleansed(&file_path_clone) {
                                    let _ = app_emit.emit(
                                        "file_processed",
                                        FileProcessedPayload {
                                            file_name: file_name.clone(),
                                            directory: dir_emit.clone(),
                                            status: "skipped".into(),
                                        },
                                    );
                                    return;
                                }

                                let ext_str = file_path_clone
                                    .extension()
                                    .unwrap()
                                    .to_str()
                                    .unwrap()
                                    .to_lowercase();
                                let status = match ext_str.as_str() {
                                    "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => {
                                        process_image(&file_path_clone)
                                    }
                                    "pdf" => process_pdf(&file_path_clone),
                                    _ => process_video(&file_path_clone),
                                };

                                let is_success = status.is_ok();
                                let status_msg = if is_success {
                                    "success".into()
                                } else {
                                    format!("error: {}", status.err().unwrap_or_default())
                                };

                                let _ = app_emit.emit(
                                    "file_processed",
                                    FileProcessedPayload {
                                        file_name: file_name.clone(),
                                        directory: dir_emit.clone(),
                                        status: status_msg,
                                    },
                                );

                                if is_success {
                                    mark_cleansed(&file_path_clone);
                                }
                            });
                        }
                    }
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| e.to_string())?;

    watcher
        .watch(Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    watchers_lock.insert(path.clone(), watcher);
    Ok(())
}

// --- Tauri Commands ---

#[tauri::command]
async fn add_directory(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    start_watcher(&app, &state, path.clone())?;
    scan_and_clean_existing_files(&app, path);
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
    image::open(path).map_err(|e| format!("Corrupted or invalid format: {}", e))?;

    // If it opens successfully, save it (which strips the metadata)
    // We use the original path to overwrite it
    let img = image::open(path).map_err(|e| e.to_string())?;
    img.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn process_pdf(path: &PathBuf) -> Result<(), String> {
    use lopdf::Document;
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    let mut info_dict = lopdf::Dictionary::new();
    info_dict.set(
        "PoistoCleansed",
        lopdf::Object::String(b"true".to_vec(), lopdf::StringFormat::Literal),
    );
    doc.trailer
        .set("Info", lopdf::Object::Dictionary(info_dict));

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
            "-y",
            "-i",
            path_str,
            "-map_metadata",
            "-1",
            "-metadata",
            "poisto_cleansed=true",
            "-c:v",
            "copy",
            "-c:a",
            "copy",
            &temp_path,
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
            active_processing: Arc::new(Mutex::new(HashSet::new())),
        })
        .setup(|app| {
            let state = app.state::<AppState>();
            let dirs_to_watch = state
                .watchers
                .lock()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();

            for dir in dirs_to_watch {
                let _ = start_watcher(app.handle(), &state, dir.clone());
                scan_and_clean_existing_files(app.handle(), dir);
            }
            Ok(())
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
