.PHONY: build run test clean fmt lint install uninstall corpus

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

# Differential check against the Python scripts this crate replaces. Needs
# python3 and a checkout of otaviocc/dotfiles's claude/.claude/skills/.
# Usage: make corpus SKILLS=/path/to/dotfiles/claude/.claude/skills
corpus:
	tests/corpus/diff_check.sh $(SKILLS)
