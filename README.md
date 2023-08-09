# Axum Opentelemetry Middelware

A middleware for axum that allows users to get insight into which endpoints are being hit how often for how long as well as registering their own metrics.

Currently it only supports outputting metrics to prometheus.
For usage please reference the docs or the examples folder.

By default the following metrics are tracked:
* `http.requests.duration.seconds` (labels: endpoint, method, status): the (internal) request duration for all tracked endpoints
* `http.requests.total` (labels: endpoint, method, status): the amount of requests for all tracked endpoints
* `http.mismatched.requests.total` (labels: none): the amount of attempted requests that didn't hit a registered endpoint

## Lints

We have plenty of lints in `lints.toml` that we use. Cargo currently does not natively support an extra file for lints, so we use `cargo-lints`. To check everything with our lints, run this locally:

```sh
cargo lints clippy --workspace --all-targets
```

and this in your IDE:
```sh
cargo lints clippy --workspace --all-targets --message-format=json
```

A few lints are commented out in `lints.toml`. This is because they should not be enabled by default, because e.g. they have false positives. However, they can be very useful sometimes.

## Pre-commit usage

1. If not installed, install with your package manager, or `pip install --user pre-commit`
2. Run `pre-commit autoupdate` to update the pre-commit config to use the newest template
3. Run `pre-commit install` to install the pre-commit hooks to your local environment


