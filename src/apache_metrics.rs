use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct ApacheMetrics {
	pub requests_total: Family<(&'static str, String), Counter>
}

impl ApacheMetrics {
	pub fn new() -> (Registry, ApacheMetrics) {
		let mut registry = <Registry>::default();
		
		let requests_total = Family::<(&'static str, String), Counter>::default();
		registry.register("apache_requests", "Number of received requests", Box::new(requests_total.clone()));
		
		let metrics = ApacheMetrics {
			requests_total
		};
		
		return (registry, metrics);
	}
}
