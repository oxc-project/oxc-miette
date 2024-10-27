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
  cargo binstall bacon cargo-insta typos-cli cargo-shear -y

# When ready, run the same CI commands
ready:
  git diff --exit-code --quiet
  typos
  cargo fmt
  cargo check
  cargo test
  cargo clippy
  cargo doc
  git status