//! TODO: crate documentation

#![deny(
	missing_docs,
	trivial_casts,
	trivial_numeric_casts,
	unused_extern_crates,
	unused_import_braces,
	unused_qualifications
)]
#![warn(missing_debug_implementations, dead_code, clippy::unwrap_used, clippy::expect_used)]

/// Write a hello world message
pub fn hello_world() {
	println!("Hello, world!")
}
