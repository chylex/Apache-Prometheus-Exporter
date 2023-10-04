use std::env;
use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Mutex;

use tokio::signal;

use crate::metrics::Metrics;
use crate::web::WebServer;

mod logs;
mod metrics;
mod web;

const ACCESS_LOG_FILE_PATTERN: &str = "ACCESS_LOG_FILE_PATTERN";
const ERROR_LOG_FILE_PATTERN: &str = "ERROR_LOG_FILE_PATTERN";

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
	
	let access_log_files = match logs::find_log_files(ACCESS_LOG_FILE_PATTERN, "access log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let error_log_files = match logs::find_log_files(ERROR_LOG_FILE_PATTERN, "error log") {
		Some(files) => files,
		None => return ExitCode::FAILURE,
	};
	
	let server = match WebServer::try_bind(SocketAddr::new(bind_ip, 9240)) {
		Some(server) => server,
		None => return ExitCode::FAILURE
	};
	
	let (metrics_registry, metrics) = Metrics::new();
	
	if !logs::start_log_watcher(access_log_files, error_log_files, metrics).await {
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
