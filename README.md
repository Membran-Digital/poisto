# 🧹 Poisto

**High-performance, real-time metadata scrubber for images, videos, and PDFs.**

Poisto (Finnish for *"removal"* or *"deletion"*) is a blazing-fast desktop application that automatically strips EXIF data, PDF info, and video metadata from your files the moment they hit your watched directories. 

Built with **Tauri v2**, **React**, and **Rust**, Poisto combines a buttery-smooth UI with raw, multi-threaded backend performance and zero-friction setup.

---

## ✨ Current Features

### 🔥 Core Functionality
- **👁️ Real-Time Watchdog:** Uses OS-level file system events (`notify`) to detect new files the millisecond they are dropped or downloaded.
- **📂 Multi-Directory Support:** Watch your `Downloads`, `Desktop`, and custom folders simultaneously.
- **🎯 Smart Bulk Scan:** Automatically scans and cleans existing unprocessed files when you add a new directory or restart the app.
- **🎬 Zero-Copy Video Stripping:** Leverages FFmpeg's stream-copy (`-c:v copy`) to strip video metadata instantly without re-encoding (a 1GB video takes ~2 seconds).
- **🖼️ Broad Format Support:** 
  - **Images:** JPG, JPEG, PNG, WebP, GIF, BMP
  - **Videos:** MP4, MOV, AVI, MKV, WebM
  - **Documents:** PDF (strips both legacy Info dictionaries and modern XMP metadata)

### 🛡️ Smart & Safe
- **🔍 Intelligent Flagging:** Uses native OS metadata (Windows ADS, macOS xattr, Linux extended attributes) to mark files as cleansed without creating extra sidecar files.
- **📄 PDF Persistence:** Embeds flags directly into PDF metadata, ensuring cleansed status survives cross-platform file transfers.
- **⏱️ Dynamic Write Detection:** Polls file size every 100ms to detect when the OS finishes writing large files, preventing corrupted reads.
- **🔄 Infinite-Loop Prevention:** Safely overwrites files in-place without triggering the file watcher into an endless processing loop.
- **🧹 Temp File Cleanup:** Automatically cleans up temporary files and ignores its own processing artifacts.

### ⚡ Performance & Architecture
- **🚀 Zero-Friction FFmpeg:** Auto-detects and downloads the correct FFmpeg binary for your platform and architecture (Windows, macOS Intel/Apple Silicon, Linux x86_64/ARM) on first use.
- **📊 Persistent Stats Tracking:** Tracks and displays total files sanitized by category (Images, PDFs, Videos) with real-time UI updates.
- **🎯 Thread Isolation:** Heavy processing offloaded to spawned background threads, ensuring the main file-watcher event loop never blocks.
- **🔒 Safe State Management:** Uses `Arc<Mutex<T>>` to share state between Tauri async runtime and background watcher threads with zero deadlocks.

### 🎨 User Experience
- **📱 Fully Responsive UI:** Modern, dark-mode dashboard that adapts beautifully from mobile to ultrawide monitors.
- **📜 Live Activity Feed:** Real-time processing logs with color-coded status badges (Cleaned, Skipped, Error).
- **🏆 Achievement Dashboard:** Persistent trophy counter showing your privacy impact with per-category breakdowns.
- **💾 Zero Configuration:** Works out of the box with sensible defaults and automatic setup.

---

## 🛠️ Tech Stack

| Layer | Technology |
| :--- | :--- |
| **Frontend** | React 18, TypeScript, Tailwind CSS, Vite |
| **Backend** | Rust, Tauri v2, Tokio, Rayon |
| **File Watching** | `notify` (OS-level event listener) |
| **Media Processing** | `image` crate, `lopdf` crate, FFmpeg (auto-downloaded) |
| **Tooling** | Bun (Package manager & dev server) |

---

## 🚀 Getting Started

### Prerequisites
1. **Rust & Cargo:** [Install Rust](https://www.rust-lang.org/tools/install)
2. **Bun:** [Install Bun](https://bun.sh/)
3. **Tauri Prerequisites:** Follow the [official Tauri setup guide](https://tauri.app/start/prerequisites/) for your OS.

**Note:** FFmpeg is **not required** to be installed manually. Poisto will automatically download the correct binary for your system on first video processing.

### Installation & Running

```bash
# 1. Clone the repository
git clone https://github.com/Membran-Digital/poisto.git
cd poisto

# 2. Install frontend dependencies
bun install

# 3. Install Rust backend dependencies
cd src-tauri
cargo build

# 4. Run the app in development mode
cd ..
bun run tauri dev