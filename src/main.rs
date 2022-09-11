use tokio::signal;
use tokio::sync::mpsc;

use crate::log_file_pattern::parse_log_file_pattern_from_env;
use crate::log_watcher::read_logs_task;

mod log_file_pattern;
mod log_watcher;

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
	
	let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();
	tokio::spawn(read_logs_task(log_files, shutdown_send.clone()));
	
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
