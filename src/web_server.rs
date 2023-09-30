use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyper::{Body, Error, header, Method, Request, Response, Server, StatusCode};
use hyper::http::Result;
use hyper::server::Builder;
use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;

const MAX_BUFFER_SIZE: usize = 1024 * 32;

pub struct WebServer {
	builder: Builder<AddrIncoming>,
}

impl WebServer {
	//noinspection HttpUrlsUsage
	pub fn try_bind(addr: SocketAddr) -> Option<WebServer> {
		println!("[WebServer] Starting web server on {0} with metrics endpoint: http://{0}/metrics", addr);
		let builder = match Server::try_bind(&addr) {
			Ok(builder) => builder,
			Err(e) => {
				println!("[WebServer] Could not bind to {}: {}", addr, e);
				return None;
			}
		};
		
		let builder = builder.tcp_keepalive(Some(Duration::from_secs(60)));
		let builder = builder.http1_only(true);
		let builder = builder.http1_keepalive(true);
		let builder = builder.http1_max_buf_size(MAX_BUFFER_SIZE);
		let builder = builder.http1_header_read_timeout(Duration::from_secs(10));
		
		Some(WebServer { builder })
	}
	
	pub async fn serve(self, metrics_registry: Mutex<Registry>) {
		let metrics_registry = Arc::new(metrics_registry);
		let service = make_service_fn(move |_| {
			let metrics_registry = Arc::clone(&metrics_registry);
			async move {
				Ok::<_, Error>(service_fn(move |req| handle_request(req, Arc::clone(&metrics_registry))))
			}
		});
		
		if let Err(e) = self.builder.serve(service).await {
			println!("[WebServer] Error starting web server: {}", e);
		}
	}
}

async fn handle_request(req: Request<Body>, metrics_registry: Arc<Mutex<Registry>>) -> Result<Response<Body>> {
	if req.method() == Method::GET && req.uri().path() == "/metrics" {
		metrics_handler(Arc::clone(&metrics_registry)).await
	} else {
		Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty())
	}
}

//noinspection SpellCheckingInspection
async fn metrics_handler(metrics_registry: Arc<Mutex<Registry>>) -> Result<Response<Body>> {
	match encode_metrics(metrics_registry) {
		MetricsEncodeResult::Ok(buf) => {
			Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE, "application/openmetrics-text; version=1.0.0; charset=utf-8").body(Body::from(buf))
		}
		MetricsEncodeResult::FailedAcquiringRegistryLock => {
			println!("[WebServer] Failed acquiring lock on registry.");
			Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty())
		}
		MetricsEncodeResult::FailedEncodingMetrics(e) => {
			println!("[WebServer] Error encoding metrics: {}", e);
			Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty())
		}
	}
}

enum MetricsEncodeResult {
	Ok(String),
	FailedAcquiringRegistryLock,
	FailedEncodingMetrics(fmt::Error),
}

fn encode_metrics(metrics_registry: Arc<Mutex<Registry>>) -> MetricsEncodeResult {
	let mut buf = String::new();
	
	return if let Ok(metrics_registry) = metrics_registry.lock() {
		encode(&mut buf, &metrics_registry).map_or_else(MetricsEncodeResult::FailedEncodingMetrics, |_| MetricsEncodeResult::Ok(buf))
	} else {
		MetricsEncodeResult::FailedAcquiringRegistryLock
	};
}
