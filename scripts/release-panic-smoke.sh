#!/usr/bin/env bash
# Spec 01 acceptance #7, PARTIAL. `cargo test` always builds the `test` profile, which is
# `panic = "unwind"` regardless of what [profile.release] says -- so a plain test suite would stay
# green even if someone later set `panic = "abort"` on release. This runs the actual `--release`
# binary and checks it returns typed 400s (not a crash) for the two malformed-input cases
# acceptance #7 names.
#
# What this does NOT prove: both inputs fail at decode(), before convert()'s catch_unwind boundary
# (§2 hardening #3) is ever reached, so they'd return the same 400 and the process would survive
# identically under panic = "abort" -- this script would stay green through that exact regression.
# convert() has no reachable panic today (the ingest module's deny-lints rule out unwrap/expect/
# panic/indexing), so there's no real payload that exercises catch_unwind itself; see
# ingest::handler::tests::catch_conversion_panic_maps_a_panic_to_err_instead_of_unwinding for that
# boundary's unit-level proof instead. Closing this gap for real needs a feature-gated panic
# injection point in convert(), built only for this script -- not done, ponytail: skip until a
# reachable panic path actually exists to guard.
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release --quiet

BIN="target/release/spanfall"
ADDR="127.0.0.1:4318"

"$BIN" &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
    if curl -s -o /dev/null -m 1 "http://$ADDR/v1/traces" -X POST -d ''; then
        break
    fi
    sleep 0.1
done

status=$(curl -s -o /dev/null -w '%{http_code}' -m 3 -X POST "http://$ADDR/v1/traces" \
    -H 'Content-Type: application/x-protobuf' --data-binary $'\xff\xff\xff')
if [ "$status" != "400" ]; then
    echo "FAIL: truncated protobuf returned $status, expected 400"
    exit 1
fi

status=$(curl -s -o /dev/null -w '%{http_code}' -m 3 -X POST "http://$ADDR/v1/traces" \
    -H 'Content-Type: application/json' -d '{not json')
if [ "$status" != "400" ]; then
    echo "FAIL: malformed JSON returned $status, expected 400"
    exit 1
fi

if ! kill -0 "$PID" 2>/dev/null; then
    echo "FAIL: release binary did not survive the malformed-input smoke test"
    exit 1
fi

echo "OK: release binary returned typed 400s for both malformed inputs and stayed alive"
