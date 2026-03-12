.PHONY: build test lint clean install

build:
	cargo build

test:
	cargo test

lint:
	cargo clippy -- -D warnings
	cargo fmt --check

clean:
	cargo clean

install:
	cargo install --path .
