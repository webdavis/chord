set shell := ["bash", "-cu"]

fmt:
  cargo fmt --all

fmt-check:
  cargo fmt --all --check

lint:
  cargo clippy --locked --workspace --all-targets -- -D warnings

doc:
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

test:
  cargo test --locked --workspace

gates: fmt-check lint doc test
