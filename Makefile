.PHONY: check fmt lint test snapshots audit deny

check: fmt lint test snapshots audit deny

fmt:
	cd apps/tui && cargo fmt

lint:
	cd apps/tui && cargo clippy -- -D warnings

test:
	cd apps/tui && cargo test

snapshots:
	cd apps/tui && cargo insta pending-snapshots

audit:
	cd apps/tui && cargo audit

deny:
	cd apps/tui && cargo deny check --config ../../deny.toml
