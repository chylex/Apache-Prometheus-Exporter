use std::collections::HashMap;
use std::io;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use linemux::MuxedLines;
use tokio::sync::mpsc::UnboundedSender;

use crate::ApacheMetrics;
use crate::log_file_pattern::LogFilePath;

pub async fn read_logs_task(log_files: Vec<LogFilePath>, metrics: ApacheMetrics, shutdown_send: UnboundedSender<()>) {
	if let Err(error) = read_logs(log_files, metrics).await {
		println!("[LogWatcher] Error reading logs: {}", error);
		shutdown_send.send(()).unwrap();
	}
}

async fn read_logs(log_files: Vec<LogFilePath>, metrics: ApacheMetrics) -> io::Result<()> {
	let mut file_reader = MuxedLines::new()?;
	let mut label_lookup: HashMap<PathBuf, &String> = HashMap::new();
	
	for log_file in &log_files {
		let lookup_key = file_reader.add_file(&log_file.path).await?;
		label_lookup.insert(lookup_key, &log_file.label);
	}
	
	if log_files.is_empty() {
		println!("[LogWatcher] No log files provided.");
		return Err(Error::from(ErrorKind::Unsupported));
	}
	
	println!("[LogWatcher] Watching {} log file(s).", log_files.len());
	
	loop {
		let event_result = file_reader.next_line().await?;
		if let Some(event) = event_result {
			match label_lookup.get(event.source()) {
				Some(label) => {
					println!("[LogWatcher] Received line from \"{}\": {}", label, event.line());
				}
				None => {
					println!("[LogWatcher] Received line from unknown file: {}", event.source().display());
				}
			}
		}
	}
}
