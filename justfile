# Justfile for netlistx-rs
# https://github.com/casey/just

# Default recipe
default: test

# Run all tests
test:
    cargo test --all-features --workspace

# Run tests with output
test-verbose:
    cargo test --all-features --workspace -- --nocapture

# Run a single test by name
test-one TEST_NAME:
    cargo test {{TEST_NAME}} -- --nocapture

# Format code
fmt:
    cargo fmt --all

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Run clippy lints
clippy:
    cargo clippy --all-targets --all-features --workspace

# Run all checks (fmt, clippy, test)
check: fmt clippy test

# Build documentation
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace

# Build in release mode
build:
    cargo build --release

# Run benchmarks
bench:
    cargo bench --all-features

# Run an example
run-example EXAMPLE:
    cargo run --example {{EXAMPLE}}

# Run all examples
examples:
    cargo run --example basic_usage
    cargo run --example builder_pattern
    cargo run --example partitioning_example

# Clean build artifacts
clean:
    cargo clean

# Install development dependencies
setup:
    cargo install cargo-udeps

# Check for unused dependencies
udeps:
    cargo udeps --all-features --workspace

# Full CI pipeline (requires installed tools)
ci: fmt-check clippy test doc udeps