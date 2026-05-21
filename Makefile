.PHONY: check check-types fmt install-hooks bump-version update release-tag

dev:
	cargo run

run-release:
	cargo run --release

check:
	./scripts/check.sh

check-types:
	./scripts/check-types.sh

fmt:
	cargo fmt --all

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
