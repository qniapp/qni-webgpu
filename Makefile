.PHONY: check fmt lint test audit deny

check: fmt lint test audit deny

fmt:
	cd apps/tui && cargo fmt

lint:
	cd apps/tui && cargo clippy -- -D warnings

test:
	cd apps/tui && cargo test

audit:
	cd apps/tui && cargo audit

deny:
	cd apps/tui && cargo deny check --config ../../deny.toml
