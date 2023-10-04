use log_file_watcher::{LogFileKind, LogWatcherConfiguration};

use crate::logs::log_file_pattern::{LogFilePath, parse_log_file_pattern_from_env};
use crate::metrics::Metrics;

mod access_log_parser;
mod filesystem_watcher;
mod log_file_pattern;
mod log_file_watcher;

pub fn find_log_files(environment_variable_name: &str, log_kind: &str) -> Option<Vec<LogFilePath>> {
	let log_file_pattern = match parse_log_file_pattern_from_env(environment_variable_name) {
		Ok(pattern) => pattern,
		Err(error) => {
			println!("Error: {}", error);
			return None;
		}
	};
	
	let log_files = match log_file_pattern.search() {
		Ok(files) => files,
		Err(error) => {
			println!("Error searching {} files: {}", log_kind, error);
			return None;
		}
	};
	
	if log_files.is_empty() {
		println!("Found no matching {} files.", log_kind);
		return None;
	}
	
	for log_file in &log_files {
		println!("Found {} file: {} (label \"{}\")", log_kind, log_file.path.display(), log_file.label);
	}
	
	Some(log_files)
}

pub async fn start_log_watcher(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: Metrics) -> bool {
	let mut watcher = LogWatcherConfiguration::new();
	
	for log_file in access_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Access);
	}
	
	for log_file in error_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Error);
	}
	
	watcher.start(&metrics).await
}
