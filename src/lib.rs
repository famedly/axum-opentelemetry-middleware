//! Opentelemetry meter middleware for axum.
//! See [RecorderMiddlewareBuilder] for usage information.

use std::{
	fmt,
	sync::Arc,
	task::{Context, Poll},
};

use axum::{
	body::Body,
	extract::MatchedPath,
	http::{Request, StatusCode},
	response::Response,
	Extension,
};
use futures::future::BoxFuture;
use opentelemetry::{
	global,
	metrics::{Counter, Histogram, Meter, MeterProvider as _},
	KeyValue,
};
use opentelemetry_sdk::{metrics::MeterProvider, Resource};
use prometheus::{Encoder, Registry, TextEncoder};
use tower::{Layer, Service};

/// Filter function type.
type FilterFunction = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

/// Builder for the metric recorder middleware.
///
/// Usage example:
/// ```
/// use std::sync::Arc;
/// use axum::{
///     routing::get, Extension, Router
/// };
/// // Create middleware builder
/// let metrics_middleware = axum_opentelemetry_middleware::RecorderMiddlewareBuilder::new("🦊")
/// // Optionally set a filter function which returns true if the request should be logged
///     .filter_function(&|endpoint, _method| endpoint != ":blubb/uwu"); //
/// // Initialize our own metric (optional)
/// let fox_counter = metrics_middleware.meter.u64_counter("fox.counter").init();
/// // Build the middleware
/// let metrics_middleware = metrics_middleware.build();
///
/// let app: Router = Router::new()
///     .route("/metrics", get(axum_opentelemetry_middleware::metrics_endpoint)) // register /metrics endpoint for prometheus
///     .layer(metrics_middleware) // register middleware
///     .layer(Extension(fox_counter)); // (optional) add our own counter as an extension
/// ```
pub struct RecorderMiddlewareBuilder {
	/// The prometheus registry.
	registry: Registry,
	/// Optional function for determining if an endpoint should be recorded or
	/// not
	filter_function: Option<FilterFunction>,
	/// Meter for people to register their own metrics
	pub meter: Meter,
}

impl fmt::Debug for RecorderMiddlewareBuilder {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RecorderMiddlewareBuilder")
			.field("registry", &self.registry)
			.field("meter", &self.meter)
			.field("filter_function", &self.filter_function.is_some())
			.finish()
	}
}

impl RecorderMiddlewareBuilder {
	/// Creates the builder for the optentelemetry middleware
	#[must_use]
	pub fn new(service_name: &str) -> Self {
		let registry = Registry::new();
		#[allow(clippy::expect_used)]
		let exporter = opentelemetry_prometheus::exporter()
			.with_registry(registry.clone())
			.build()
			.expect("Exporter should build");
		let provider = MeterProvider::builder()
			.with_resource(Resource::new(vec![KeyValue::new(
				"service.name",
				service_name.to_owned(),
			)]))
			.with_reader(exporter)
			.build();
		let meter = provider.meter("axum-opentelemetry");

		global::set_meter_provider(provider);

		Self { registry, meter, filter_function: None }
	}

	/// Registers a function for filtering which endpoints should be logged and
	/// which not The argument takes the route and method of the
	/// request and returns true if the middleware should track it
	#[must_use]
	pub fn filter_function(
		mut self,
		function: &'static (dyn Fn(&str, &str) -> bool + Send + Sync),
	) -> Self {
		self.filter_function = Some(Arc::new(function));
		self
	}

	#[must_use]
	/// Builds the middleware data
	pub fn build(self) -> RecorderMiddleware {
		RecorderMiddleware::new(self.meter, self.registry, self.filter_function)
	}
}

/// The actual RecorderMiddleware, has to added to an axum Router via
/// `Router::layer(middleware)` See [RecorderMiddlewareBuilder] for more details
#[derive(Clone)]
pub struct RecorderMiddleware {
	/// The prometheus registry
	registry: Registry,
	/// Metric tracking the duration of each request
	http_requests_duration_seconds: Histogram<f64>,
	/// Metric tracking amounts of taken requests
	http_requests_total: Counter<u64>,
	/// Amount of http requests being made to unknown paths
	http_unmatched_requests_total: Counter<u64>,
	/// User provided function
	filter_function: Option<FilterFunction>,
}

impl fmt::Debug for RecorderMiddleware {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RecorderMiddleware")
			.field("http_requests_duration_seconds", &self.http_requests_duration_seconds)
			.field("http_requests_total", &self.http_requests_total)
			.field("http_unmatched_requests_total", &self.http_unmatched_requests_total)
			.field("filter_function", &self.filter_function.is_some())
			.finish()
	}
}

impl RecorderMiddleware {
	/// Create the actual middleware struct
	#[must_use]
	fn new(meter: Meter, registry: Registry, filter_function: Option<FilterFunction>) -> Self {
		// ValueRecorder == prometheus histogram
		let http_requests_duration_seconds =
			meter.f64_histogram("http.requests.duration.seconds").init();

		let http_requests_total = meter.u64_counter("http.requests.total").init();
		let http_unmatched_requests_total =
			meter.u64_counter("http.mismatched.requests.total").init();

		Self {
			registry,
			http_requests_duration_seconds,
			http_requests_total,
			http_unmatched_requests_total,
			filter_function,
		}
	}
}

/// Service for RecorderMiddleware
#[derive(Clone, Debug)]
pub struct RecorderMiddlewareService<S> {
	/// Inner service
	inner: S,
	/// Our RecorderMiddleware as data
	data: RecorderMiddleware,
}

impl<S> Layer<S> for RecorderMiddleware {
	type Service = RecorderMiddlewareService<S>;

	fn layer(&self, inner: S) -> Self::Service {
		RecorderMiddlewareService { inner, data: self.clone() }
	}
}

impl<S> Service<Request<Body>> for RecorderMiddlewareService<S>
where
	S: Service<Request<Body>, Response = Response> + Send + 'static,
	S::Future: Send + 'static,
{
	type Response = S::Response;
	type Error = S::Error;
	type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx)
	}

	fn call(&mut self, mut req: Request<Body>) -> Self::Future {
		let data = self.data.clone();
		let method = req.method().as_str().to_owned();
		req.extensions_mut().insert(data.registry);
		let matched_path =
			req.extensions().get::<MatchedPath>().map(|path| path.as_str().to_owned());
		let future = self.inner.call(req);
		Box::pin(async move {
			let Some(path) = matched_path else {
				data.http_unmatched_requests_total.add(1, &[]);
				return future.await;
			};

			let start = time::OffsetDateTime::now_utc();

			let resp = future.await?;

			if let Some(filter_function) = &data.filter_function {
				if !(filter_function)(&path, &method) {
					// If filter function returns false we do not want to track this request
					return Ok(resp);
				}
			}

			let time_taken = time::OffsetDateTime::now_utc() - start;

			let status = resp.status().as_str().to_owned();

			let attributes = [
				KeyValue::new("endpoint", path),
				KeyValue::new("method", method),
				KeyValue::new("status", status),
			];

			data.http_requests_duration_seconds.record(time_taken.as_seconds_f64(), &attributes);
			data.http_requests_total.add(1, &attributes);

			Ok(resp)
		})
	}
}

/// Endpoint that returns prometheus metrics
/// usually should be on get("/metrics")
#[allow(clippy::unused_async)] // needs to be async else axum complains
pub async fn metrics_endpoint(
	Extension(registry): Extension<Registry>,
) -> Result<String, (StatusCode, String)> {
	let encoder = TextEncoder::new();
	let metric_families = registry.gather();
	let mut result = Vec::new();
	encoder
		.encode(&metric_families, &mut result)
		.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode data".to_owned()))?;

	String::from_utf8(result).map_err(|_| {
		(StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode string from data".to_owned())
	})
}
