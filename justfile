#!/usr/bin/env -S just --justfile

set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
  @just --list -u

alias r := ready

# Make sure you have cargo-binstall installed.
# You can download the pre-compiled binary from <https://github.com/cargo-bins/cargo-binstall#installation>
# or install via `cargo install cargo-binstall`
# Initialize the project by installing all the necessary tools.
init:
  cargo binstall watchexec-cli cargo-insta typos-cli cargo-shear@1.13.1 -y

# When ready, run the same CI commands
ready:
  git diff --exit-code --quiet
  typos
  cargo shear --check-test-targets
  cargo fmt
  cargo check
  cargo clippy
  cargo test --features fancy
  cargo check --benches --features fancy
  cargo doc
  git status

# Run the benchmarks (fixtures are downloaded from benchmark-files on first run)
bench:
  cargo bench --features fancy

watch *args='':
  watchexec {{args}}

watch-check:
  just watch "'cargo check; cargo clippy'"
