use std::collections::HashMap;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use linemux::{Line, MuxedLines};
use tokio::sync::mpsc::UnboundedSender;

use crate::ApacheMetrics;
use crate::log_file_pattern::LogFilePath;

#[derive(Copy, Clone, PartialEq)]
enum LogFileKind {
	Access,
	Error,
}

struct LogFileInfo<'a> {
	pub kind: LogFileKind,
	pub label: &'a String,
}

impl<'a> LogFileInfo<'a> {
	fn get_label_set(&self) -> (&'static str, String) {
		return ("file", self.label.clone());
	}
}

pub async fn watch_logs_task(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: ApacheMetrics, shutdown_send: UnboundedSender<()>) {
	if let Err(error) = watch_logs(access_log_files, error_log_files, metrics).await {
		println!("[LogWatcher] Error reading logs: {}", error);
		shutdown_send.send(()).unwrap();
	}
}

struct LogWatcher<'a> {
	reader: MuxedLines,
	files: HashMap<PathBuf, LogFileInfo<'a>>,
}

impl<'a> LogWatcher<'a> {
	fn new() -> io::Result<LogWatcher<'a>> {
		return Ok(LogWatcher {
			reader: MuxedLines::new()?,
			files: HashMap::new(),
		});
	}
	
	fn count_files_of_kind(&self, kind: LogFileKind) -> usize {
		return self.files.values().filter(|info| info.kind == kind).count();
	}
	
	async fn add_file(&mut self, log_file: &'a LogFilePath, kind: LogFileKind) -> io::Result<()> {
		let lookup_key = self.reader.add_file(&log_file.path).await?;
		self.files.insert(lookup_key, LogFileInfo { kind, label: &log_file.label });
		Ok(())
	}
	
	async fn start_watching(&mut self, metrics: &ApacheMetrics) -> io::Result<()> {
		if self.files.is_empty() {
			println!("[LogWatcher] No log files provided.");
			return Err(Error::from(ErrorKind::Unsupported));
		}
		
		println!("[LogWatcher] Watching {} access log file(s) and {} error log file(s).", self.count_files_of_kind(LogFileKind::Access), self.count_files_of_kind(LogFileKind::Error));
		
		for metadata in self.files.values() {
			let label_set = metadata.get_label_set();
			let _ = metrics.requests_total.get_or_create(&label_set);
			let _ = metrics.errors_total.get_or_create(&label_set);
		}
		
		loop {
			if let Some(event) = self.reader.next_line().await? {
				self.handle_line(event, metrics);
			}
		}
	}
	
	fn handle_line(&mut self, event: Line, metrics: &ApacheMetrics) {
		match self.files.get(event.source()) {
			Some(metadata) => {
				let label = metadata.label;
				let (kind, family) = match metadata.kind {
					LogFileKind::Access => ("access log", &metrics.requests_total),
					LogFileKind::Error => ("error log", &metrics.errors_total),
				};
				
				println!("[LogWatcher] Received {} line from \"{}\": {}", kind, label, event.line());
				family.get_or_create(&metadata.get_label_set()).inc();
			}
			None => {
				println!("[LogWatcher] Received line from unknown file: {}", event.source().display());
			}
		}
	}
}

async fn watch_logs(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: ApacheMetrics) -> io::Result<()> {
	let mut watcher = LogWatcher::new()?;
	
	for log_file in &access_log_files {
		watcher.add_file(log_file, LogFileKind::Access).await?;
	}
	
	for log_file in &error_log_files {
		watcher.add_file(log_file, LogFileKind::Error).await?;
	}
	
	watcher.start_watching(&metrics).await?;
	Ok(())
}
