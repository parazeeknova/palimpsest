#!/usr/bin/env bash
set -euo pipefail

export RUSTFLAGS="${RUSTFLAGS:-} --cfg=rustix_use_libc"

ROW() {
  printf '\033[48;2;0;255;255;38;2;0;0;0m  %-70s  \033[0m\n' "$1"
}

ROW "running tests"
cargo test --all-targets --all-features
ROW "all tests passed"
