use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use rayon::prelude::*;


#[derive(Clone, Serialize)]
struct ProgressPayload {
    current: usize,
    total: usize,
    file_name: String,
    status: String,
}

#[tauri::command]
fn scan_directory(path: String) -> Vec<String> {
    let extensions = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "pdf", "mp4", "mov", "avi", "mkv", "webm"];

    WalkDir::new(&path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| extensions.contains(&s.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect()
}

#[tauri::command]
async fn process_files(app: AppHandle, files: Vec<String>) -> Result<(), String> {
    let total = files.len();
    let app_clone = app.clone();

    // Offload heavy CPU work to a blocking thread pool so we don't freeze the async runtime
    tokio::task::spawn_blocking(move || {
        files.par_iter().enumerate().for_each(|(index, file_path)| {
            let path = PathBuf::from(file_path);
            let ext = path.extension().unwrap().to_str().unwrap().to_lowercase();
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();

            let status = match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => process_image(&path),
                "pdf" => process_pdf(&path),
                _ => process_video(&path),
            };

            let _ = app_clone.emit("progress", ProgressPayload {
                current: index + 1,
                total,
                file_name,
                status: if status.is_ok() { "success".into() } else { format!("error: {}", status.err().unwrap_or_default()) },
            });
        });
    }).await.map_err(|e| e.to_string())?;

    Ok(())
}

// --- Processors ---

fn process_image(path: &PathBuf) -> Result<(), String> {
    // The image crate naturally drops EXIF/metadata when re-encoding
    let img = image::open(path).map_err(|e| e.to_string())?;
    img.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn process_pdf(path: &PathBuf) -> Result<(), String> {
    use lopdf::Document;

    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    // 1. Remove the legacy Info dictionary from the trailer (Author, Title, Dates, etc.)
    doc.trailer.remove(b"Info");

    // 2. Remove XMP metadata from the Catalog (Root object)
    // We get the Root reference directly from the trailer
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

    // Stream copy (no re-encoding) + strip metadata
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

    // Overwrite original with temp file
    std::fs::rename(&temp_path, path_str).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        e.to_string()
    })?;

    Ok(())
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![scan_directory, process_files])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
