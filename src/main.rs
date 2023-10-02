use std::env;
use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Mutex;

use tokio::signal;

use crate::apache_metrics::ApacheMetrics;
use crate::log_file_pattern::{LogFilePath, parse_log_file_pattern_from_env};
use crate::log_watcher::start_log_watcher;
use crate::web_server::WebServer;

mod apache_metrics;
mod fs_watcher;
mod log_file_pattern;
mod log_parser;
mod log_watcher;
mod web_server;

const ACCESS_LOG_FILE_PATTERN: &str = "ACCESS_LOG_FILE_PATTERN";
const ERROR_LOG_FILE_PATTERN: &str = "ERROR_LOG_FILE_PATTERN";

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
	
	Some(log_files)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
	let host = env::var("HTTP_HOST").unwrap_or(String::from("127.0.0.1"));
	let bind_ip = match IpAddr::from_str(&host) {
		Ok(addr) => addr,
		Err(_) => {
			println!("Invalid HTTP host: {}", host);
			return ExitCode::FAILURE;
		}
	};
	
	println!("Initializing exporter...");
	
	let access_log_files = match find_log_files(ACCESS_LOG_FILE_PATTERN, "access log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let error_log_files = match find_log_files(ERROR_LOG_FILE_PATTERN, "error log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let server = match WebServer::try_bind(SocketAddr::new(bind_ip, 9240)) {
		Some(server) => server,
		None => return ExitCode::FAILURE
	};
	
	let (metrics_registry, metrics) = ApacheMetrics::new();
	
	if !start_log_watcher(access_log_files, error_log_files, metrics).await {
		return ExitCode::FAILURE;
	}
	
	tokio::spawn(server.serve(Mutex::new(metrics_registry)));
	
	match signal::ctrl_c().await {
		Ok(_) => {
			println!("Received CTRL-C, shutting down...");
			ExitCode::SUCCESS
		}
		Err(e) => {
			println!("Error registering CTRL-C handler: {}", e);
			ExitCode::FAILURE
		}
	}
}
