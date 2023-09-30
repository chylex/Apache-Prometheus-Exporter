use std::{fmt, str};
use std::sync::Mutex;
use std::time::Duration;

use actix_web::{App, HttpResponse, HttpServer, Result, web};
use actix_web::dev::Server;
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;

//noinspection HttpUrlsUsage
pub fn create_web_server(host: &str, port: u16, metrics_registry: Mutex<Registry>) -> Server {
	let metrics_registry = web::Data::new(metrics_registry);
	
	let server = HttpServer::new(move || {
		App::new()
			.app_data(metrics_registry.clone())
			.service(web::resource("/metrics").route(web::get().to(metrics_handler)))
	});
	
	let server = server.keep_alive(Duration::from_secs(60));
	let server = server.shutdown_timeout(0);
	let server = server.disable_signals();
	let server = server.workers(1);
	let server = server.bind((host, port));

	println!("[WebServer] Starting web server on {0}:{1} with metrics endpoint: http://{0}:{1}/metrics", host, port);
	server.unwrap().run()
}

pub async fn run_web_server(server: Server) {
	if let Err(e) = server.await {
		println!("[WebServer] Error running web server: {}", e);
	}
}

//noinspection SpellCheckingInspection
async fn metrics_handler(metrics_registry: web::Data<Mutex<Registry>>) -> Result<HttpResponse> {
	let response = match encode_metrics(metrics_registry) {
		MetricsEncodeResult::Ok(buf) => {
			HttpResponse::Ok().content_type("application/openmetrics-text; version=1.0.0; charset=utf-8").body(buf)
		}
		MetricsEncodeResult::FailedAcquiringRegistryLock => {
			println!("[WebServer] Failed acquiring lock on registry.");
			HttpResponse::InternalServerError().body("")
		}
		MetricsEncodeResult::FailedEncodingMetrics(e) => {
			println!("[WebServer] Error encoding metrics: {}", e);
			HttpResponse::InternalServerError().body("")
		}
	};
	
	Ok(response)
}

enum MetricsEncodeResult {
	Ok(String),
	FailedAcquiringRegistryLock,
	FailedEncodingMetrics(fmt::Error),
}

fn encode_metrics(metrics_registry: web::Data<Mutex<Registry>>) -> MetricsEncodeResult {
	let mut buf = String::new();
	
	return if let Ok(metrics_registry) = metrics_registry.lock() {
		encode(&mut buf, &metrics_registry).map_or_else(MetricsEncodeResult::FailedEncodingMetrics, |_| MetricsEncodeResult::Ok(buf))
	} else {
		MetricsEncodeResult::FailedAcquiringRegistryLock
	}
}
