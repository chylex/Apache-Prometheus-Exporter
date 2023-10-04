use std::fmt;
use std::sync::{Arc, Mutex};

use hyper::{Body, http, Response, StatusCode};
use hyper::header::CONTENT_TYPE;
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;

//noinspection SpellCheckingInspection
const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

pub async fn handle(metrics_registry: Arc<Mutex<Registry>>) -> http::Result<Response<Body>> {
	match try_encode(metrics_registry) {
		MetricsEncodeResult::Ok(buf) => {
			Response::builder().status(StatusCode::OK).header(CONTENT_TYPE, METRICS_CONTENT_TYPE).body(Body::from(buf))
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

fn try_encode(metrics_registry: Arc<Mutex<Registry>>) -> MetricsEncodeResult {
	let mut buf = String::new();
	
	return if let Ok(metrics_registry) = metrics_registry.lock() {
		encode(&mut buf, &metrics_registry).map_or_else(MetricsEncodeResult::FailedEncodingMetrics, |_| MetricsEncodeResult::Ok(buf))
	} else {
		MetricsEncodeResult::FailedAcquiringRegistryLock
	};
}
