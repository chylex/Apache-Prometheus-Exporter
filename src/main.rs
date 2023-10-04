use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Mutex;

use anyhow::{anyhow, Context};
use tokio::signal;

use crate::metrics::Metrics;
use crate::web::WebServer;

mod logs;
mod metrics;
mod web;

const ACCESS_LOG_FILE_PATTERN: &str = "ACCESS_LOG_FILE_PATTERN";
const ERROR_LOG_FILE_PATTERN: &str = "ERROR_LOG_FILE_PATTERN";

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
	let host = env::var("HTTP_HOST").unwrap_or(String::from("127.0.0.1"));
	let bind_ip = IpAddr::from_str(&host).map_err(|_| anyhow!("Invalid HTTP host: {}", host))?;
	
	println!("Initializing exporter...");
	
	let access_log_files = logs::find_log_files(ACCESS_LOG_FILE_PATTERN, "access log").context("Could not find access log files")?;
	let error_log_files = logs::find_log_files(ERROR_LOG_FILE_PATTERN, "error log").context("Could not find error log files")?;
	
	let server = WebServer::try_bind(SocketAddr::new(bind_ip, 9240)).context("Could not configure web server")?;
	let (metrics_registry, metrics) = Metrics::new();
	
	logs::start_log_watcher(access_log_files, error_log_files, metrics).await.context("Could not start watching logs")?;
	tokio::spawn(server.serve(Mutex::new(metrics_registry)));
	
	signal::ctrl_c().await.with_context(|| "Could not register CTRL-C handler")?;
	println!("Received CTRL-C, shutting down...");
	Ok(())
}
