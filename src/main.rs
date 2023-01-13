use std::env;
use std::process::ExitCode;
use std::sync::Mutex;

use tokio::signal;
use tokio::sync::mpsc;

use crate::apache_metrics::ApacheMetrics;
use crate::log_file_pattern::{LogFilePath, parse_log_file_pattern_from_env};
use crate::log_watcher::watch_logs_task;
use crate::web_server::{create_web_server, run_web_server};

mod apache_metrics;
mod log_file_pattern;
mod log_parser;
mod log_watcher;
mod web_server;

const ACCESS_LOG_FILE_PATTERN: &'static str = "ACCESS_LOG_FILE_PATTERN";
const ERROR_LOG_FILE_PATTERN: &'static str = "ERROR_LOG_FILE_PATTERN";

fn find_log_files(environment_variable_name: &str, log_kind: &str) -> Option<Vec<LogFilePath>> {
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
	
	return Some(log_files);
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
	let host = env::var("HTTP_HOST").unwrap_or(String::from("127.0.0.1"));
	
	println!("Initializing exporter...");
	
	let access_log_files = match find_log_files(ACCESS_LOG_FILE_PATTERN, "access log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let error_log_files = match find_log_files(ERROR_LOG_FILE_PATTERN, "error log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let (metrics_registry, metrics) = ApacheMetrics::new();
	let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();
	
	tokio::spawn(watch_logs_task(access_log_files, error_log_files, metrics.clone(), shutdown_send.clone()));
	tokio::spawn(run_web_server(create_web_server(host.as_str(), 9240, Mutex::new(metrics_registry))));
	
	drop(shutdown_send);
	
	tokio::select! {
		_ = signal::ctrl_c() => {
			println!("Received CTRL-C, shutting down...")
		}
		
		_ = shutdown_recv.recv() => {
			println!("Shutting down...");
		}
	}
	
	ExitCode::SUCCESS
}
