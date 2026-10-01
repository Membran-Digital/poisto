# 🧹 Poisto

**High-performance, real-time metadata scrubber for images, videos, and PDFs.**

![Poisto Promo](/image.png)

Poisto (Finnish for *"removal"* or *"deletion"*) is a blazing-fast desktop application that automatically strips EXIF data, PDF info, and video metadata from your files the moment they hit your watched directories. 

Built with **Tauri v2**, **React**, and **Rust**, Poisto combines a buttery-smooth UI with raw, multi-threaded backend performance.

---

## ✨ Current Features (State of the Project)

- **👁️ Real-Time Watchdog:** Uses OS-level file system events (`notify`) to detect new files the millisecond they are dropped or downloaded.
- **📂 Multi-Directory Support:** Watch your `Downloads`, `Desktop`, and custom folders simultaneously.
- **⚡ High-Performance Backend:** 
  - Uses `rayon` for parallel CPU-bound image/PDF processing.
  - Uses `tokio` for async I/O and background thread spawning.
- ** Zero-Copy Video Stripping:** Leverages FFmpeg's stream-copy (`-c:v copy`) to strip video metadata instantly without re-encoding (a 1GB video takes ~2 seconds).
- **🖼️ Broad Format Support:** 
  - **Images:** JPG, JPEG, PNG, WebP, GIF, BMP
  - **Videos:** MP4, MOV, AVI, MKV, WebM
  - **Documents:** PDF (strips both legacy Info dictionaries and modern XMP metadata)
- **🛡️ Smart Write Detection:** Dynamically waits for the OS to finish writing large files to disk before processing, preventing corrupted reads during drag-and-drop.
- **🔄 Infinite-Loop Prevention:** Safely overwrites files in-place without triggering the file watcher into an endless processing loop.
- **🎨 Sleek UI:** Modern, light-theme React + Tailwind interface with live activity logs and animated elements.

---

## 🛠️ Tech Stack

| Layer | Technology |
| :--- | :--- |
| **Frontend** | React 18, TypeScript, Tailwind CSS, Vite |
| **Backend** | Rust, Tauri v2, Tokio, Rayon |
| **File Watching** | `notify` (OS-level event listener) |
| **Media Processing** | `image` crate, `lopdf` crate, FFmpeg (CLI) |
| **Tooling** | Bun (Package manager & dev server) |

---

## 🚀 Getting Started

### Prerequisites
1. **Rust & Cargo:** [Install Rust](https://www.rust-lang.org/tools/install)
2. **Bun:** [Install Bun](https://bun.sh/)
3. **FFmpeg:** Required for video processing. 
   - *Windows:* `winget install ffmpeg` or download from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/)
   - *macOS:* `brew install ffmpeg`
   - *Linux:* `sudo apt install ffmpeg`
4. **Tauri Prerequisites:** Follow the [official Tauri setup guide](https://tauri.app/start/prerequisites/) for your OS.

### Installation & Running

```bash
# 1. Clone the repository
git clone https://github.com/yourusername/poisto.git
cd poisto

# 2. Install frontend dependencies
bun install

# 3. Install Rust backend dependencies
cd src-tauri
cargo build

# 4. Run the app in development mode
cd ..
bun run tauri dev
