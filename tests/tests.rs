#![allow(
	clippy::dbg_macro,
	clippy::expect_used,
	clippy::missing_docs_in_private_items,
	clippy::print_stderr,
	clippy::print_stdout,
	clippy::unwrap_used,
	clippy::unused_async
)]

use std::future::poll_fn;

use axum::{body::Body, extract::Request, response::IntoResponse, routing::get, Extension, Router};
use http_body_util::BodyExt;
use opentelemetry::metrics::Counter;
use pretty_assertions::assert_eq;
use regex::Regex;
use tower::Service;

async fn visible_fox(Extension(fox_counter): Extension<Counter<u64>>) -> impl IntoResponse {
	// need to keep track of foxes
	fox_counter.add(1, &[]);
	"Heya!"
}

async fn shy_fox(Extension(fox_counter): Extension<Counter<u64>>) -> impl IntoResponse {
	// still counts tho
	fox_counter.add(1, &[]);
	"*hides*"
}

#[tokio::test]
async fn basic_functionality() -> Result<(), Box<dyn std::error::Error>> {
	let metrics_middleware = axum_opentelemetry_middleware::RecorderMiddlewareBuilder::new("🦊")
		.filter_function(&|endpoint, _method| endpoint != "/shy_fox"); // Skip the endpoint /shy_fox
	let fox_counter = metrics_middleware.meter.u64_counter("fox.counter").build();
	let metrics_middleware = metrics_middleware.build();

	let mut service: Router = Router::new()
		.route("/metrics", get(axum_opentelemetry_middleware::metrics_endpoint))
		.route("/visible_fox", get(visible_fox))
		.route("/shy_fox", get(shy_fox))
		.layer(metrics_middleware)
		.layer(Extension(fox_counter));

	poll_fn(|cx| <Router as tower::Service<Request>>::poll_ready(&mut service, cx)).await?;

	for _ in 0..5 {
		service.call(axum::http::Request::get("/visible_fox").body(Body::empty())?).await?;
	}

	service.call(axum::http::Request::get("/shy_fox").body(Body::empty())?).await?;

	for _ in 0..10 {
		// Endpoint doesn't exist so it should increase the unmatched requests counter
		service.call(axum::http::Request::get("/unmatched_fox").body(Body::empty())?).await?;
	}

	let resp = service.call(axum::http::Request::get("/metrics").body(Body::empty())?).await?;
	let body: String =
		String::from_utf8(resp.collect().await.unwrap().to_bytes().to_vec()).unwrap();
	let re = Regex::new(r"[0-9]\.[0-9]*")?; // remove all floats as they will vary from run to run
	let body = re.replace_all(&body, "FLOAT");

	assert_eq!(include_str!("basic_metrics_output.txt"), body);
	Ok(())
}
