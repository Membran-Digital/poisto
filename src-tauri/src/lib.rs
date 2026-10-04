use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

// --- FFmpeg Auto-Discovery & Auto-Download (Zero Dependencies) ---

fn get_ffmpeg_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    // 1. Check system PATH first (respects users who already have it)
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok(PathBuf::from("ffmpeg"));
    }

    // 2. Check if we already downloaded it to app data dir
    let ffmpeg_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("ffmpeg");

    let binary_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let ffmpeg_bin = ffmpeg_dir.join(binary_name);

    if ffmpeg_bin.exists() {
        return Ok(ffmpeg_bin);
    }

    // 3. Auto-download static FFmpeg build (one-time)
    download_ffmpeg(&ffmpeg_dir)?;

    if ffmpeg_bin.exists() {
        Ok(ffmpeg_bin)
    } else {
        Err("FFmpeg download completed but binary not found".to_string())
    }
}

fn download_ffmpeg(target_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create ffmpeg dir: {}", e))?;

    let archive_name = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        "ffmpeg_archive.zip"
    } else {
        "ffmpeg_archive.tar.xz"
    };

    // 1. Determine the correct download URL based on OS and Architecture
    let url = if cfg!(target_os = "windows") {
        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip".to_string()
    } else if cfg!(target_os = "macos") {
        "https://evermeet.cx/ffmpeg/getrelease/zip".to_string()
    } else {
        // Linux: Detect architecture (x86_64 vs aarch64/arm64 vs arm)
        let arch = std::env::consts::ARCH;
        let arch_str = match arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "arm" => "armhf",
            _ => {
                return Err(format!(
                    "Unsupported Linux architecture for auto-download: {}",
                    arch
                ))
            }
        };
        format!(
            "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-{}-static.tar.xz",
            arch_str
        )
    };

    let archive_path = target_dir.join(archive_name);
    let archive_str = archive_path.to_str().ok_or("Invalid archive path")?;
    let target_str = target_dir.to_str().ok_or("Invalid target path")?;

    // 2. Download using the most reliable tool per platform
    let download_result = if cfg!(target_os = "windows") {
        let ps_cmd = format!(
            "try {{ Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing -ErrorAction Stop; exit 0 }} catch {{ Write-Host $_.Exception.Message; exit 1 }}",
            url, archive_str.replace('\\', "\\\\")
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
            .output()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/curl")
            .args([
                "-L",
                "--fail",
                "--silent",
                "--show-error",
                "-o",
                archive_str,
                &url,
            ])
            .output()
    } else {
        // Linux: curl with wget fallback
        let curl_result = std::process::Command::new("/usr/bin/curl")
            .args([
                "-L",
                "--fail",
                "--silent",
                "--show-error",
                "-o",
                archive_str,
                &url,
            ])
            .output();

        match curl_result {
            Ok(output) if output.status.success() => Ok(output),
            _ => std::process::Command::new("/usr/bin/wget")
                .args(["-q", "-O", archive_str, &url])
                .output(),
        }
    };

    let output = download_result.map_err(|e| format!("Download command failed to start: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "FFmpeg download failed (status: {:?}). Stderr: {} Stdout: {}",
            output.status.code(),
            stderr,
            stdout
        ));
    }

    // 3. Verify the archive was actually created and has size
    match std::fs::metadata(&archive_path) {
        Ok(meta) if meta.len() > 1000 => { /* Good, archive downloaded */ }
        Ok(meta) => {
            let _ = std::fs::remove_file(&archive_path);
            return Err(format!(
                "Downloaded file too small ({} bytes), likely a redirect/error page",
                meta.len()
            ));
        }
        Err(e) => return Err(format!("Archive not found after download: {}", e)),
    }

    // 4. Extract using tar
    let extract = std::process::Command::new("tar")
        .args(["-xf", archive_str, "-C", target_str])
        .output()
        .map_err(|e| format!("tar error: {}", e))?;

    let _ = std::fs::remove_file(&archive_path);

    if !extract.status.success() {
        return Err(format!(
            "Extraction failed: {}",
            String::from_utf8_lossy(&extract.stderr)
        ));
    }

    // 5. Find and move the binary to the root of target_dir
    let binary_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let dest = target_dir.join(binary_name);

    if !dest.exists() {
        find_and_move_ffmpeg(target_dir, binary_name, &dest)?;
    }

    // 6. Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }

    Ok(())
}

fn find_and_move_ffmpeg(search_dir: &Path, binary_name: &str, dest: &Path) -> Result<(), String> {
    for entry in WalkDir::new(search_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() && entry.file_name() == binary_name {
            std::fs::copy(entry.path(), dest)
                .map_err(|e| format!("Failed to copy ffmpeg binary: {}", e))?;
            return Ok(());
        }
    }
    Err("ffmpeg binary not found inside the downloaded archive".to_string())
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
    stats: Arc<Mutex<PoistoStats>>,
}

#[derive(Clone, Serialize)]
struct FileProcessedPayload {
    file_name: String,
    directory: String,
    status: String,
}

use serde::Deserialize;

// --- Persistent Stats Tracking ---
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PoistoStats {
    pub images_cleaned: u64,
    pub pdfs_cleaned: u64,
    pub videos_cleaned: u64,
}

impl PoistoStats {
    pub fn total(&self) -> u64 {
        self.images_cleaned + self.pdfs_cleaned + self.videos_cleaned
    }

    pub fn load(app_handle: &AppHandle) -> Self {
        let path = get_stats_path(app_handle);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(stats) = serde_json::from_str(&data) {
                return stats;
            }
        }
        Self::default()
    }

    pub fn save(&self, app_handle: &AppHandle) {
        let path = get_stats_path(app_handle);
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }
}

fn get_stats_path(app_handle: &AppHandle) -> PathBuf {
    app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap())
        .join("poisto_stats.json")
}

// Helper: Record a successful clean and notify the UI
fn record_success(app_handle: &AppHandle, stats: &Arc<Mutex<PoistoStats>>, ext: &str) {
    let mut stats_lock = stats.lock().unwrap();
    match ext {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => stats_lock.images_cleaned += 1,
        "pdf" => stats_lock.pdfs_cleaned += 1,
        _ => stats_lock.videos_cleaned += 1,
    }
    let stats_clone = stats_lock.clone();
    drop(stats_lock);

    // Persist to disk
    stats_clone.save(app_handle);

    // Emit to frontend for real-time UI update
    let _ = app_handle.emit("stats_updated", stats_clone);
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
fn scan_and_clean_existing_files(
    app_handle: &AppHandle,
    stats: Arc<Mutex<PoistoStats>>,
    dir_path: String,
) {
    let extensions = [
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "pdf", "mp4", "mov", "avi", "mkv", "webm",
    ];
    let app_clone = app_handle.clone();
    let stats_clone = stats.clone();
    let dir_clone = dir_path.clone();

    std::thread::spawn(move || {
        for entry in WalkDir::new(&dir_clone).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();

            if path.to_string_lossy().contains(".poisto_tmp.") {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !extensions.contains(&ext.as_str()) {
                continue;
            }

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

            let status = match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => process_image(&path),
                "pdf" => process_pdf(&path),
                _ => process_video(&path, &app_clone),
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
                record_success(&app_clone, &stats_clone, &ext);
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
                        if (&path_str).contains(".poisto_tmp.") {
                            continue;
                        }

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
                                    _ => process_video(&file_path_clone, &app_emit),
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
    scan_and_clean_existing_files(&app, state.stats.clone(), path);
    Ok(())
}

#[tauri::command]
async fn remove_directory(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let mut watchers = state.watchers.lock().unwrap();
    watchers.remove(&path);
    Ok(())
}

#[tauri::command]
async fn get_stats(state: State<'_, AppState>) -> Result<PoistoStats, String> {
    Ok(state.stats.lock().unwrap().clone())
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

fn process_video(path: &PathBuf, app_handle: &AppHandle) -> Result<(), String> {
    let ffmpeg = get_ffmpeg_path(app_handle)?;
    let path_str = path.to_str().ok_or("Invalid path")?;

    // Extract the original extension (mp4, mov, mkv, etc.) to tell FFmpeg the output format
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("mp4");
    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Build temp path with extension at the END: "video.poisto_tmp.mp4"
    let temp_path_buf = parent.join(format!("{}.poisto_tmp.{}", file_stem, ext));
    let temp_path_str = temp_path_buf.to_str().ok_or("Invalid temp path")?;

    let output = std::process::Command::new(&ffmpeg)
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
            temp_path_str, // Extension is at the end, so FFmpeg knows the format
        ])
        .output()
        .map_err(|e| format!("ffmpeg execution error: {}", e))?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_path_buf);
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    std::fs::rename(&temp_path_buf, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path_buf);
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
            stats: Arc::new(Mutex::new(PoistoStats::default())),
        })
        .setup(|app| {
            let state = app.state::<AppState>();

            // Load persisted stats on startup
            {
                let mut stats_lock = state.stats.lock().unwrap();
                *stats_lock = PoistoStats::load(app.handle());
            }

            let dirs_to_watch = state
                .watchers
                .lock()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();

            for dir in dirs_to_watch {
                let _ = start_watcher(app.handle(), &state, dir.clone());
                scan_and_clean_existing_files(app.handle(), state.stats.clone(), dir);
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            add_directory,
            remove_directory,
            get_stats,
            get_directories
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
