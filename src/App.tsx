import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface ProcessedFile {
  file_name: string;
  directory: string;
  status: string;
}

interface PoistoStats {
  images_cleaned: number;
  pdfs_cleaned: number;
  videos_cleaned: number;
  total: number;
}

function App() {
  const [directories, setDirectories] = useState<string[]>([]);
  const [logs, setLogs] = useState<ProcessedFile[]>([]);
  const [stats, setStats] = useState<PoistoStats>({
    images_cleaned: 0,
    pdfs_cleaned: 0,
    videos_cleaned: 0,
    total: 0,
  });

  useEffect(() => {
    invoke<string[]>("get_directories").then(setDirectories).catch(console.error);
    invoke<PoistoStats>("get_stats").then((s) => {
      setStats({ ...s, total: s.images_cleaned + s.pdfs_cleaned + s.videos_cleaned });
    }).catch(console.error);
  }, []);

  useEffect(() => {
    const unlistenFiles = listen<ProcessedFile>("file_processed", (event) => {
      setLogs((prev) => [event.payload, ...prev].slice(0, 200));
    });
    const unlistenStats = listen<PoistoStats>("stats_updated", (event) => {
      const s = event.payload;
      setStats({ ...s, total: s.images_cleaned + s.pdfs_cleaned + s.videos_cleaned });
    });
    return () => {
      unlistenFiles.then((fn) => fn());
      unlistenStats.then((fn) => fn());
    };
  }, []);

  const handleAddFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      try {
        await invoke("add_directory", { path: selected });
        setDirectories((prev) => [...prev, selected]);
      } catch (error) {
        console.error("Failed to add directory:", error);
      }
    }
  };

  const handleRemoveFolder = async (path: string) => {
    try {
      await invoke("remove_directory", { path });
      setDirectories((prev) => prev.filter((d) => d !== path));
    } catch (error) {
      console.error("Failed to remove directory:", error);
    }
  };

  const formatNumber = (num: number) => num.toLocaleString();

  return (
    <div className="h-screen w-full bg-[#0d1117] text-gray-100 flex flex-col font-sans overflow-hidden">
      {/* Header */}
      <header className="shrink-0 border-b border-[#30363d] bg-[#161b22]/80 backdrop-blur-md z-10">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-cyan-500 to-blue-600 flex items-center justify-center shadow-lg shadow-cyan-500/20">
              <svg className="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </div>
            <div>
              <h1 className="text-xl font-bold tracking-tight text-white">Poisto</h1>
              <p className="text-xs text-gray-400">Automated Metadata Scrubber</p>
            </div>
          </div>
          <button
            type="button"
            onClick={handleAddFolder}
            className="flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-cyan-600 to-blue-600 hover:from-cyan-500 hover:to-blue-500 rounded-lg text-sm font-medium transition-all shadow-lg shadow-blue-900/20 active:scale-95"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            Add Folder
          </button>
        </div>
      </header>

      {/* Stats Bar */}
      <div className="shrink-0 border-b border-[#30363d] bg-[#161b22]/50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-gradient-to-br from-green-500 to-emerald-600 flex items-center justify-center shadow-lg shadow-green-500/20">
                <svg className="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div>
                <p className="text-2xl font-bold text-white tracking-tight">
                  {formatNumber(stats.total)}
                </p>
                <p className="text-xs text-gray-400 uppercase tracking-wider">Files Sanitized</p>
              </div>
            </div>

            <div className="flex items-center gap-6">
              {/* Images */}
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-purple-500/10 flex items-center justify-center">
                  <svg className="w-4 h-4 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                </div>
                <div>
                  <p className="text-lg font-semibold text-purple-300">{formatNumber(stats.images_cleaned)}</p>
                  <p className="text-[10px] text-gray-500 uppercase">Images</p>
                </div>
              </div>

              {/* PDFs */}
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-red-500/10 flex items-center justify-center">
                  <svg className="w-4 h-4 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                </div>
                <div>
                  <p className="text-lg font-semibold text-red-300">{formatNumber(stats.pdfs_cleaned)}</p>
                  <p className="text-[10px] text-gray-500 uppercase">PDFs</p>
                </div>
              </div>

              {/* Videos */}
              <div className="flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-blue-500/10 flex items-center justify-center">
                  <svg className="w-4 h-4 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
                  </svg>
                </div>
                <div>
                  <p className="text-lg font-semibold text-blue-300">{formatNumber(stats.videos_cleaned)}</p>
                  <p className="text-[10px] text-gray-500 uppercase">Videos</p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Main Content Grid */}
      <main className="flex-1 min-h-0 w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6 grid grid-cols-1 lg:grid-cols-3 gap-6">
        
        {/* Left Column: Directories */}
        <div className="lg:col-span-1 flex flex-col min-h-0">
          <div className="bg-[#161b22] rounded-xl border border-[#30363d] shadow-sm flex flex-col h-full min-h-0">
            <div className="shrink-0 p-4 border-b border-[#30363d]">
              <h2 className="text-sm font-semibold text-gray-200 uppercase tracking-wider">Watched Directories</h2>
              <p className="text-xs text-gray-500 mt-1">Files added here will be cleaned automatically.</p>
            </div>
            <div className="flex-1 min-h-0 overflow-y-auto p-3 space-y-2 custom-scrollbar">
              {directories.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-center p-6">
                  <div className="w-12 h-12 rounded-full bg-[#0d1117] flex items-center justify-center mb-3 border border-[#30363d]">
                    <svg className="w-6 h-6 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                  </div>
                  <p className="text-gray-400 text-sm font-medium">No directories watched</p>
                </div>
              ) : (
                directories.map((dir) => (
                  <div key={dir} className="group flex items-center justify-between bg-[#0d1117] p-3 rounded-lg border border-[#30363d] hover:border-cyan-500/50 transition-colors">
                    <div className="flex-1 min-w-0 mr-3">
                      <p className="text-sm font-mono text-cyan-300 truncate" title={dir}>{dir}</p>
                      <div className="flex items-center gap-1.5 mt-1.5">
                        <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse"></span>
                        <span className="text-[10px] text-gray-500 uppercase tracking-wide font-medium">Active</span>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleRemoveFolder(dir)}
                      className="opacity-0 group-hover:opacity-100 p-1.5 text-gray-500 hover:text-red-400 hover:bg-red-900/20 rounded-md transition-all"
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Right Column: Live Logs */}
        <div className="lg:col-span-2 flex flex-col min-h-0">
          <div className="bg-[#161b22] rounded-xl border border-[#30363d] shadow-sm flex flex-col h-full min-h-0">
            <div className="shrink-0 p-4 border-b border-[#30363d] flex items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold text-gray-200 uppercase tracking-wider">Live Activity</h2>
                <p className="text-xs text-gray-500 mt-1">Real-time processing status.</p>
              </div>
              <div className="flex items-center gap-2">
                <span className="flex h-2 w-2 relative">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-cyan-400 opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-cyan-500"></span>
                </span>
                <span className="text-xs text-cyan-400 font-medium">Listening</span>
              </div>
            </div>
            
            <div className="flex-1 min-h-0 overflow-y-auto p-4 font-mono text-xs custom-scrollbar bg-[#0d1117]/30">
              {logs.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-center">
                  <p className="text-gray-600 text-sm">Waiting for file activity...</p>
                </div>
              ) : (
                <div className="space-y-1.5">
                  {logs.map((log, i) => {
                    const isSuccess = log.status === "success";
                    const isSkipped = log.status === "skipped";
                    const isError = !isSuccess && !isSkipped;

                    return (
                      <div key={i} className="flex items-start gap-3 p-2 rounded-md hover:bg-[#161b22] transition-colors animate-in fade-in slide-in-from-top-2 duration-200">
                        <span className={`shrink-0 px-2 py-0.5 rounded text-[10px] font-bold uppercase min-w-[70px] text-center tracking-wide ${
                          isSuccess ? "bg-green-500/10 text-green-400 border border-green-500/20" :
                          isSkipped ? "bg-gray-500/10 text-gray-400 border border-gray-500/20" :
                          "bg-red-500/10 text-red-400 border border-red-500/20"
                        }`}>
                          {isSuccess ? "Cleaned" : isSkipped ? "Skipped" : "Error"}
                        </span>
                        <div className="flex-1 min-w-0">
                          <p className="text-gray-200 truncate font-medium">{log.file_name}</p>
                          <p className="text-gray-500 text-[10px] truncate mt-0.5">{log.directory}</p>
                        </div>
                        {isError && (
                          <span className="shrink-0 text-red-400 text-[10px] truncate max-w-[120px] text-right" title={log.status}>
                            {log.status.replace("error: ", "").split('\n')[0]}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;