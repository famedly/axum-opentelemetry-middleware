#![allow(
	clippy::dbg_macro,
	clippy::expect_used,
	clippy::missing_docs_in_private_items,
	clippy::print_stderr,
	clippy::print_stdout,
	clippy::unwrap_used,
	clippy::unused_async
)]

use axum::{
	body::{Body, HttpBody},
	response::IntoResponse,
	routing::get,
	Extension, Router,
};
use futures::future::poll_fn;
use opentelemetry::{metrics::Counter, Context};
use regex::Regex;
use tower::Service;

async fn visible_fox(Extension(fox_counter): Extension<Counter<u64>>) -> impl IntoResponse {
	// need to keep track of foxes
	let ctx = Context::current();
	fox_counter.add(&ctx, 1, &[]);
	"Heya!"
}

async fn shy_fox(Extension(fox_counter): Extension<Counter<u64>>) -> impl IntoResponse {
	// still counts tho
	let ctx = Context::current();
	fox_counter.add(&ctx, 1, &[]);
	"*hides*"
}

#[tokio::test]
async fn basic_functionality() -> Result<(), Box<dyn std::error::Error>> {
	let metrics_middleware = axum_opentelemetry_middleware::RecorderMiddlewareBuilder::new("🦊")
		.filter_function(&|endpoint, _method| endpoint != "/shy_fox"); // Skip the endpoint /shy_fox
	let fox_counter = metrics_middleware.meter.u64_counter("fox.counter").init();
	let metrics_middleware = metrics_middleware.build();

	let mut service = Router::new()
		.route("/metrics", get(axum_opentelemetry_middleware::metrics_endpoint))
		.route("/visible_fox", get(visible_fox))
		.route("/shy_fox", get(shy_fox))
		.layer(metrics_middleware)
		.layer(Extension(fox_counter));

	poll_fn(|cx| service.poll_ready(cx)).await?;

	for _ in 0..5 {
		service.call(http::Request::get("/visible_fox").body(Body::empty())?).await?;
	}

	service.call(http::Request::get("/shy_fox").body(Body::empty())?).await?;

	for _ in 0..10 {
		// Endpoint doesn't exist so it should increase the unmatched requests counter
		service.call(http::Request::get("/unmatched_fox").body(Body::empty())?).await?;
	}

	let mut resp = service.call(http::Request::get("/metrics").body(Body::empty())?).await?;
	let body = String::from_utf8(resp.data().await.unwrap()?.to_vec())?;
	let re = Regex::new(r"[0-9]\.[0-9]*")?; // remove all floats as they will vary from run to run
	let body = re.replace_all(&body, "FLOAT");

	assert_eq!(include_str!("basic_metrics_output.txt"), body);
	Ok(())
}
