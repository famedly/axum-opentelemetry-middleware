#![allow(
	clippy::dbg_macro,
	clippy::expect_used,
	clippy::missing_docs_in_private_items,
	clippy::print_stderr,
	clippy::print_stdout,
	clippy::unwrap_used,
	clippy::unused_async
)]

//! Simple usage example, prometheus can connect on localhost:3000
use std::net::SocketAddr;

use axum::{response::IntoResponse, routing::get, Extension, Router};
use opentelemetry::metrics::Counter;
use tokio::time::{sleep, Duration};

async fn uwu() -> impl IntoResponse {
	"UwU!"
}

/// Foxes need some time to be counted!
/// (Will increase the fox counter every 100ms until it counts 50 foxes)
async fn fooox(Extension(fox_counter): Extension<Counter<u64>>) -> impl IntoResponse {
	for _ in 0..50 {
		sleep(Duration::from_millis(100)).await;
		fox_counter.add(1, &[]);
	}
	"Counted lots of foxes!!!"
}

#[tokio::main]
async fn main() {
	let metrics_middleware = axum_opentelemetry_middleware::RecorderMiddlewareBuilder::new("🦊")
		.filter_function(&|endpoint, _method| endpoint != ":blubb/uwu");
	let fox_counter = metrics_middleware.meter.u64_counter("fox.counter").init();
	let metrics_middleware = metrics_middleware.build();

	let app = Router::new()
		.route("/metrics", get(axum_opentelemetry_middleware::metrics_endpoint))
		.route("/:blubb/uwu", get(uwu))
		.route("/fooox", get(fooox))
		.layer(metrics_middleware)
		.layer(Extension(fox_counter));

	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
	axum::serve(
		tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind to addr"),
		app.into_make_service(),
	)
	.await
	.expect("Failed to serve service");
}
