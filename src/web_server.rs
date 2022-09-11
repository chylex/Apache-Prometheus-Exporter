use std::str;
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

	println!("[WebServer] Starting web server on http://{}:{}", host, port);
	return server.unwrap().run();
}

pub async fn run_web_server(server: Server) {
	if let Err(e) = server.await {
		println!("[WebServer] Error running web server: {}", e);
	}
}

async fn metrics_handler(metrics_registry: web::Data<Mutex<Registry>>) -> Result<HttpResponse> {
	let mut buf = Vec::new();
	
	{
		if let Ok(metrics_registry) = metrics_registry.lock() {
			encode(&mut buf, &metrics_registry)?;
		} else {
			println!("[WebServer] Failed acquiring lock on registry.");
			return Ok(HttpResponse::InternalServerError().body(""));
		}
	}
	
	if let Ok(buf) = String::from_utf8(buf) {
		Ok(HttpResponse::Ok().content_type("application/openmetrics-text; version=1.0.0; charset=utf-8").body(buf))
	} else {
		println!("[WebServer] Failed converting buffer to UTF-8.");
		Ok(HttpResponse::InternalServerError().body(""))
	}
}
