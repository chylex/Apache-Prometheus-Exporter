use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;

type SingleLabel = [(&'static str, String); 1];

#[derive(Clone)]
pub struct ApacheMetrics {
	pub requests_total: Family<SingleLabel, Counter>,
	pub errors_total: Family<SingleLabel, Counter>
}

impl ApacheMetrics {
	pub fn new() -> (Registry, ApacheMetrics) {
		let mut registry = <Registry>::default();
		
		let metrics = ApacheMetrics {
			requests_total: Family::<SingleLabel, Counter>::default(),
			errors_total: Family::<SingleLabel, Counter>::default()
		};
		
		registry.register("apache_requests", "Number of received requests", metrics.requests_total.clone());
		registry.register("apache_errors", "Number of logged errors", metrics.errors_total.clone());
		
		(registry, metrics)
	}
}
