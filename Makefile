.PHONY: build test clean fmt lint install uninstall corpus

build:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

fmt:
	cargo fmt

lint:
	cargo clippy --all-targets -- -D warnings

install:
	cargo install --path . --locked --force

uninstall:
	cargo uninstall acervo

# The golden-corpus regression test alone (tests/corpus.rs) — the synthetic
# libraries in tests/corpus/ diffed against the checked-in snapshots in
# tests/corpus/expected/. Already covered by `make test`; this is just a
# faster way to re-run it on its own.
corpus:
	cargo test --test corpus
