build_date = `date +%Y%m%d%H%M`
commit = `git rev-parse HEAD`
version = `git rev-parse --short HEAD`

.PHONY: test

test:
	cargo test --verbose --features collector && \
	cargo test --verbose --features use_tokio --features collector --no-default-features

