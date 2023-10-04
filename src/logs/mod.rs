use std::env;
use std::env::VarError;

use anyhow::{anyhow, bail, Context, Result};

use log_file_watcher::{LogFileKind, LogWatcherConfiguration};

use crate::logs::log_file_pattern::{LogFilePath, parse_log_file_pattern_from_str};
use crate::metrics::Metrics;

mod access_log_parser;
mod filesystem_watcher;
mod log_file_pattern;
mod log_file_watcher;

pub fn find_log_files(environment_variable_name: &str, log_kind: &str) -> Result<Vec<LogFilePath>> {
	let log_file_pattern_str = env::var(environment_variable_name).map_err(|err| match err {
		VarError::NotPresent => anyhow!("Environment variable {} must be set", environment_variable_name),
		VarError::NotUnicode(_) => anyhow!("Environment variable {} contains invalid characters", environment_variable_name)
	})?;
	
	let log_file_pattern = parse_log_file_pattern_from_str(&log_file_pattern_str).with_context(|| format!("Could not parse pattern: {}", log_file_pattern_str))?;
	let log_files = log_file_pattern.search().with_context(|| format!("Could not search files: {}", log_file_pattern_str))?;
	
	if log_files.is_empty() {
		bail!("No files match pattern: {}", log_file_pattern_str);
	}
	
	for log_file in &log_files {
		println!("Found {} file: {} (label \"{}\")", log_kind, log_file.path.display(), log_file.label);
	}
	
	Ok(log_files)
}

pub async fn start_log_watcher(access_log_files: Vec<LogFilePath>, error_log_files: Vec<LogFilePath>, metrics: Metrics) -> Result<()> {
	let mut watcher = LogWatcherConfiguration::new();
	
	for log_file in access_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Access);
	}
	
	for log_file in error_log_files.into_iter() {
		watcher.add_file(log_file, LogFileKind::Error);
	}
	
	watcher.start(&metrics).await
}
