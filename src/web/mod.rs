use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyper::{Body, Error, Method, Request, Response, Server, StatusCode};
use hyper::http::Result;
use hyper::server::Builder;
use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use prometheus_client::registry::Registry;

mod metrics_endpoint;

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
		metrics_endpoint::handle(Arc::clone(&metrics_registry)).await
	} else {
		Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty())
	}
}
