import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface ProcessedFile {
	file_name: string;
	directory: string;
	status: string;
}

function App() {
	const [directories, setDirectories] = useState<string[]>([]);
	const [logs, setLogs] = useState<ProcessedFile[]>([]);

	// Fetch currently watched directories on load
	useEffect(() => {
		invoke<string[]>("get_directories")
			.then(setDirectories)
			.catch(console.error);
	}, []);

	// Listen for real-time file processing events
	useEffect(() => {
		const unlisten = listen<ProcessedFile>("file_processed", (event) => {
			setLogs((prev) => [event.payload, ...prev].slice(0, 100));
		});
		return () => {
			unlisten.then((fn) => fn());
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

	return (
		<div className="min-h-screen bg-[#0d1117] text-gray-100 flex flex-col items-center p-8 font-sans">
			<div className="w-full max-w-3xl bg-[#161b22] rounded-2xl shadow-2xl p-8 border border-[#30363d]">
				{/* Header */}
				<div className="text-center mb-8">
					<h1 className="text-4xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 to-blue-500 tracking-tight">
						Poisto
					</h1>
					<p className="text-gray-400 mt-2 text-sm">
						Real-time automated metadata scrubber.
					</p>
				</div>

				{/* Directory Management */}
				<div className="mb-6">
					<div className="flex items-center justify-between mb-3">
						<h2 className="text-lg font-semibold text-gray-200">
							Watched Directories
						</h2>
						<button
							type="button"
							onClick={handleAddFolder}
							className="px-4 py-2 bg-gradient-to-r from-cyan-600 to-blue-600 hover:from-cyan-500 hover:to-blue-500 rounded-lg text-sm font-medium transition-all shadow-lg shadow-blue-900/20"
						>
							+ Add Folder
						</button>
					</div>

					<div className="bg-[#0d1117] rounded-lg border border-[#30363d] p-4 min-h-[100px]">
						{directories.length === 0 ? (
							<p className="text-gray-500 text-center text-sm py-4">
								No directories being watched. Add a folder to begin.
							</p>
						) : (
							<ul className="space-y-2">
								{directories.map((dir) => (
									<li
										key={dir}
										className="flex items-center justify-between bg-[#161b22] p-3 rounded-lg border border-[#30363d]"
									>
										<span
											className="text-sm font-mono text-cyan-300 truncate mr-4"
											title={dir}
										>
											{dir}
										</span>
										<button
											type="button"
											onClick={() => handleRemoveFolder(dir)}
											className="text-red-400 hover:text-red-300 text-sm font-medium px-2 py-1 hover:bg-red-900/20 rounded transition-colors"
										>
											Stop
										</button>
									</li>
								))}
							</ul>
						)}
					</div>
				</div>

				{/* Live Logs */}
				<div>
					<h2 className="text-lg font-semibold text-gray-200 mb-3">
						Live Activity
					</h2>
					<div className="bg-[#0d1117] rounded-lg p-4 h-72 overflow-y-auto font-mono text-xs border border-[#30363d]">
						{logs.length === 0 ? (
							<p className="text-gray-600 text-center mt-24">
								Waiting for new files...
							</p>
						) : (
							logs.map((log, i) => (
								<div key={i} className="mb-2 flex items-start gap-2">
									<span
										className={`px-1.5 py-0.5 rounded text-[10px] font-bold uppercase ${log.status === "success" ? "bg-green-900/30 text-green-400" : "bg-red-900/30 text-red-400"}`}
									>
										{log.status === "success" ? "Cleaned" : "Error"}
									</span>
									<div className="flex-1 truncate">
										<span className="text-gray-200">{log.file_name}</span>
										<span className="text-gray-500 ml-2 text-[10px]">
											({log.directory})
										</span>
									</div>
								</div>
							))
						)}
					</div>
				</div>
			</div>
			<p className="text-gray-600 text-xs mt-6">
				Supports Images (WebP, JPG, PNG), Videos (MP4, MOV), and PDFs. Requires
				FFmpeg for video.
			</p>
		</div>
	);
}

export default App;
