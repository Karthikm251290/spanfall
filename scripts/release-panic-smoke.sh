#!/usr/bin/env bash
# Spec 01 acceptance #7. `cargo test` always builds the `test` profile, which is
# `panic = "unwind"` regardless of what [profile.release] says -- so a plain test suite would stay
# green even if someone later set `panic = "abort"` on release. This runs the actual `--release`
# binary, built with `--features panic-injection` (see Cargo.toml and ingest/convert.rs), and
# checks:
#   - the two malformed-input cases acceptance #7 names return typed 400s (decode()-level rejects)
#   - a span named `__spanfall_panic_injection__` reaches convert()'s catch_unwind boundary (§2
#     hardening #3) and comes back as a 500 (IngestError::ConversionPanicked), not a dead process
# The last check is the one that actually discriminates panic = "unwind" from "abort": flip
# [profile.release] to abort and re-run this script -- it should fail (process dies, curl gets a
# connection error instead of 500) -- then revert.
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release --quiet --features panic-injection

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

status=$(curl -s -o /dev/null -w '%{http_code}' -m 3 -X POST "http://$ADDR/v1/traces" \
    -H 'Content-Type: application/json' \
    -d '{"resourceSpans":[{"scopeSpans":[{"spans":[{"traceId":"01010101010101010101010101010101","spanId":"0202020202020202","name":"__spanfall_panic_injection__","startTimeUnixNano":"1","endTimeUnixNano":"2","status":{"code":0}}]}]}]}')
if [ "$status" != "500" ]; then
    echo "FAIL: panic-injection span returned $status, expected 500 (ConversionPanicked)"
    exit 1
fi

if ! kill -0 "$PID" 2>/dev/null; then
    echo "FAIL: release binary did not survive catch_unwind catching the injected panic"
    exit 1
fi

echo "OK: release binary returned typed rejects, survived an injected convert() panic via catch_unwind"
