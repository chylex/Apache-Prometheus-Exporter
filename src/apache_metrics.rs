use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct ApacheMetrics {}

impl ApacheMetrics {
	pub fn new() -> (Registry, ApacheMetrics) {
		let mut registry = <Registry>::default();
		let metrics = ApacheMetrics {};
		return (registry, metrics);
	}
}
