use std::collections::HashMap;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use linemux::{Line, MuxedLines};
use tokio::sync::mpsc::UnboundedSender;

use crate::{ApacheMetrics, log_parser};
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
	
	fn handle_line(&self, event: Line, metrics: &ApacheMetrics) {
		if let Some(file) = self.files.get(event.source()) {
			match file.kind {
				LogFileKind::Access => self.handle_access_log_line(event.line(), file, metrics),
				LogFileKind::Error => self.handle_error_log_line(event.line(), file, metrics),
			}
		} else {
			println!("[LogWatcher] Received line from unknown file: {}", event.source().display());
		}
	}
	
	fn handle_access_log_line(&self, line: &str, file: &LogFileInfo, metrics: &ApacheMetrics) {
		match log_parser::AccessLogLineParts::parse(line) {
			Ok(parts) => {
				println!("[LogWatcher] Received access log line from \"{}\": {}", file.label, parts)
			}
			Err(err) => {
				println!("[LogWatcher] Received access log line from \"{}\" with invalid format ({:?}): {}", file.label, err, line)
			}
		}
		
		metrics.requests_total.get_or_create(&file.get_label_set()).inc();
	}
	
	fn handle_error_log_line(&self, line: &str, file: &LogFileInfo, metrics: &ApacheMetrics) {
		println!("[LogWatcher] Received error log line from \"{}\": {}", file.label, line);
		metrics.errors_total.get_or_create(&file.get_label_set()).inc();
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
