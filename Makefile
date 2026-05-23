.PHONY: check check-types fmt install-hooks bump-version update release-tag clean-local

dev:
	cargo run

run-release:
	cargo run --release

clean-local:
	rm -rf ~/.local/share/palimpsest

check:
	./scripts/check.sh

check-types:
	./scripts/check-types.sh

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all --check

install-hooks:
	./scripts/install-hooks.sh

bump-version:
	./scripts/bump-version.rs

update:
	cargo update

release-tag:
	./scripts/tag-release.sh

build:
	cargo build --release
