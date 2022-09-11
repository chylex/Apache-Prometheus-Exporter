use std::env;
use std::sync::Mutex;

use tokio::signal;
use tokio::sync::mpsc;

use crate::apache_metrics::ApacheMetrics;
use crate::log_file_pattern::parse_log_file_pattern_from_env;
use crate::log_watcher::read_logs_task;
use crate::web_server::{create_web_server, run_web_server};

mod log_file_pattern;
mod log_watcher;
mod apache_metrics;
mod web_server;

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let host = env::var("HTTP_HOST").unwrap_or(String::from("127.0.0.1"));
	
	println!("Initializing exporter...");
	
	let log_file_pattern = match parse_log_file_pattern_from_env() {
		Ok(pattern) => pattern,
		Err(error) => {
			println!("Error: {}", error);
			return;
		}
	};
	
	let log_files = match log_file_pattern.search() {
		Ok(files) => files,
		Err(error) => {
			println!("Error searching log files: {}", error);
			return;
		}
	};
	
	if log_files.is_empty() {
		println!("Found no matching log files.");
		return;
	}
	
	for log_file in &log_files {
		println!("Found log file: {} (label \"{}\")", log_file.path.display(), log_file.label);
	}
	
	let (metrics_registry, metrics) = ApacheMetrics::new();
	let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();
	
	tokio::spawn(read_logs_task(log_files, metrics.clone(), shutdown_send.clone()));
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
}
