#!/bin/sh
# What the improvements lane runs on h@backplane (through bp-build.sh): the focused tests, then the debug hub and client for the e2e.
cargo test --manifest-path native/host/Cargo.toml settings::issue_report 2>&1 | grep -E 'FAILED|panicked|test result: (ok. [1-9]|FAILED)|^error' | tail -20
cargo test --manifest-path native/host/Cargo.toml settings::diagnostics_http 2>&1 | grep -E 'FAILED|panicked|test result: (ok. [1-9]|FAILED)|^error' | tail -5
cargo test --manifest-path native/host/Cargo.toml settings::improvements 2>&1 | grep -E 'FAILED|panicked|test result: (ok. [1-9]|FAILED)|^error' | tail -5
cargo test --manifest-path native/gpui/Cargo.toml improv 2>&1 | grep -E 'FAILED|panicked|test result: (ok. [1-9]|FAILED)|^error' | tail -20
cargo build --manifest-path native/host/Cargo.toml --bin backplane-hub 2>&1 | tail -3
cargo build --manifest-path native/gpui/Cargo.toml 2>&1 | tail -3
ls -la .target/debug/backplane-hub .target/debug/backplane-gpui
