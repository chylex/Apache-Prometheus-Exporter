use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;

type SingleLabel = (&'static str, String);

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
		
		registry.register("apache_requests", "Number of received requests", Box::new(metrics.requests_total.clone()));
		registry.register("apache_errors", "Number of logged errors", Box::new(metrics.errors_total.clone()));
		
		return (registry, metrics);
	}
}
