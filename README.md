# 🧹 Poisto

**High-performance, real-time metadata scrubber for images, videos, and PDFs.**

Poisto (Finnish for *"removal"* or *"deletion"*) is a blazing-fast desktop application that automatically strips EXIF data, PDF info, and video metadata from your files the moment they hit your watched directories. 

Built with **Tauri v2**, **React**, and **Rust**, Poisto combines a buttery-smooth UI with raw, multi-threaded backend performance.

---

## ✨ Current Features (State of the Project)

- **👁️ Real-Time Watchdog:** Uses OS-level file system events (`notify`) to detect new files the millisecond they are dropped or downloaded.
- **📂 Multi-Directory Support:** Watch your `Downloads`, `Desktop`, and custom folders simultaneously.
- **⚡ High-Performance Backend:** 
  - Uses `rayon` for parallel CPU-bound image/PDF processing.
  - Uses `tokio` for async I/O and background thread spawning.
- **🎬 Zero-Copy Video Stripping:** Leverages FFmpeg's stream-copy (`-c:v copy`) to strip video metadata instantly without re-encoding (a 1GB video takes ~2 seconds).
- **🖼️ Broad Format Support:** 
  - **Images:** JPG, JPEG, PNG, WebP, GIF, BMP
  - **Videos:** MP4, MOV, AVI, MKV, WebM
  - **Documents:** PDF (strips both legacy Info dictionaries and modern XMP metadata)
- **🛡️ Smart Write Detection:** Dynamically waits for the OS to finish writing large files to disk before processing, preventing corrupted reads during drag-and-drop.
- **🔄 Infinite-Loop Prevention:** Safely overwrites files in-place without triggering the file watcher into an endless processing loop.
- **🎨 Sleek UI:** Modern, dark-mode React + Tailwind interface with live activity logs.

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
```

### Building for Production

```bash
bun run tauri build
```
The compiled binary will be located in `src-tauri/target/release/bundle/`.

---

## 🏗️ Architecture Highlights

- **`Arc<Mutex<T>>` State Management:** The app uses atomic reference-counted mutexes to share state between the Tauri async runtime and the background `notify` file watcher threads, ensuring zero deadlocks.
- **Dynamic File Readiness:** Instead of hardcoded `sleep()` timers, Poisto polls the file size every 100ms. Once the size stops growing, it knows the OS has finished writing the file.
- **Thread Isolation:** Heavy processing is offloaded to spawned background threads so the main file-watcher event loop never blocks, ensuring no dropped events even when 50 files are dropped at once.

---

## 🗺️ Roadmap & Next Steps

We are actively looking to expand Poisto. Here is what's coming next:

### 🟢 Short Term
- [ ] **Backup Toggle:** Add a UI toggle to create a `.bak` copy of the original file before overwriting.
- [ ] **Bundled FFmpeg:** Ship a static FFmpeg binary inside the Tauri app so users don't need to install it manually.
- [ ] **System Tray Integration:** Allow the app to minimize to the system tray and run silently in the background.
- [ ] **Auto-start on Boot:** Option to launch Poisto on system startup for continuous background protection.

### 🟡 Medium Term
- [ ] **Custom Ignore Rules:** Allow users to define rules (e.g., "Ignore files under 1MB", "Ignore files containing 'draft' in the name").
- [ ] **CLI Version (`poisto-cli`):** A headless, terminal-only version for servers, CI/CD pipelines, and power users.
- [ ] **Batch Processing Mode:** Bring back the manual "Select Folder & Scan All" mode for cleaning existing archives.

### 🔵 Long Term
- [ ] **Extended Format Support:** Add support for HEIC, RAW camera files, and Office documents (DOCX, XLSX).
- [ ] **Deep Metadata Viewer:** A tool to *view* the metadata before deciding to strip it.
- [ ] **Cross-Platform Mobile:** Leverage Tauri v2 to bring Poisto to Android and iOS.

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome! 

1. Fork the repo
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📜 License

Distributed under the MIT License. See `LICENSE` for more information.

---

<p align="center">
  Made with 🦀 Rust and ❤️ by the open-source community.
</p>