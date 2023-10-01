use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::ApacheMetrics;
use crate::log_file_pattern::LogFilePath;

#[derive(Copy, Clone, PartialEq)]
enum LogFileKind {
	Access,
	Error,
}

struct LogFile {
	pub path: PathBuf,
	pub metadata: LogFileMetadata,
}

struct LogFileMetadata {
	pub kind: LogFileKind,
	pub label: String,
}

impl LogFileMetadata {
	fn get_label_set(&self) -> [(&'static str, String); 1] {
		[("file", self.label.clone())]
	}
}

pub async fn start_log_watcher(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: ApacheMetrics) -> bool {
	let mut watcher = LogWatcher::new();
	
	for log_file in access_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Access);
	}
	
	for log_file in error_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Error);
	}
	
	watcher.start(&metrics).await
}

struct LogWatcher {
	files: Vec<LogFile>,
}

impl LogWatcher {
	fn new() -> LogWatcher {
		LogWatcher { files: Vec::new() }
	}
	
	fn count_files_of_kind(&self, kind: LogFileKind) -> usize {
		return self.files.iter().filter(|info| info.metadata.kind == kind).count();
	}
	
	fn add_file(&mut self, log_file: LogFilePath, kind: LogFileKind) {
		let path = log_file.path;
		let label = log_file.label;
		let metadata = LogFileMetadata { kind, label };
		self.files.push(LogFile { path, metadata });
	}
	
	async fn start(self, metrics: &ApacheMetrics) -> bool {
		if self.files.is_empty() {
			println!("[LogWatcher] No log files provided.");
			return false;
		}
		
		println!("[LogWatcher] Watching {} access log file(s) and {} error log file(s).", self.count_files_of_kind(LogFileKind::Access), self.count_files_of_kind(LogFileKind::Error));
		
		for file in self.files.into_iter() {
			let metadata = file.metadata;
			let label_set = metadata.get_label_set();
			let _ = metrics.requests_total.get_or_create(&label_set);
			let _ = metrics.errors_total.get_or_create(&label_set);
			
			let command = Command::new("tail")
				.arg("-q") // Don't print file names.
				.arg("-F") // Follow rotations.
				.arg("-n").arg("0") // Start from end.
				.arg(&file.path)
				.env_clear()
				.stdin(Stdio::null())
				.stdout(Stdio::piped())
				.stderr(Stdio::null())
				.spawn();
			
			let mut process = match command {
				Ok(process) => process,
				Err(error) => {
					println!("[LogWatcher] Error spawning tail process for file \"{}\": {}", file.path.to_string_lossy(), error);
					return false;
				}
			};
			
			let stdout = match process.stdout.take() {
				Some(stdout) => stdout,
				None => {
					println!("[LogWatcher] No output handle in tail process for file: {}", file.path.to_string_lossy());
					return false;
				}
			};
			
			let mut output_reader = BufReader::new(stdout).lines();
			let metrics = metrics.clone();
			
			tokio::spawn(async move {
				loop {
					match output_reader.next_line().await {
						Ok(maybe_line) => match maybe_line {
							Some(line) => handle_line(&metadata, line, &metrics),
							None => break,
						},
						Err(e) => {
							println!("[LogWatcher] Error reading from file \"{}\": {}", metadata.label, e);
							break;
						}
					}
				}
			});
		}
		
		true
	}
}

fn handle_line(metadata: &LogFileMetadata, line: String, metrics: &ApacheMetrics) {
	let (kind, family) = match metadata.kind {
		LogFileKind::Access => ("access log", &metrics.requests_total),
		LogFileKind::Error => ("error log", &metrics.errors_total),
	};
	
	println!("[LogWatcher] Received {} line from \"{}\": {}", kind, metadata.label, line);
	family.get_or_create(&metadata.get_label_set()).inc();
}
