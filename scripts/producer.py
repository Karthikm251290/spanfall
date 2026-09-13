#!/usr/bin/env python3
"""spec 00 T3 — throwaway trace producer.

Emits ONE trace with a controllable span count to an OTLP endpoint (gRPC
:4317 or HTTP :4318), as a realistic parent/child tree rather than a flat
list, so it actually exercises the incumbents' tree expansion.

NOT M1's benchmark generator (spec 02) — deliberately a separate, disposable
script. Any language was fine per spec 00; this uses the official
OpenTelemetry Python SDK rather than hand-rolling OTLP protobuf encoding.

Usage:
    pip install -r scripts/requirements.txt
    python3 scripts/producer.py --spans 40000 --protocol grpc --endpoint localhost:4317
    python3 scripts/producer.py --spans 10000 --protocol http --endpoint http://localhost:4318/v1/traces
"""

import argparse
import random
import time

from opentelemetry import trace
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

SERVICES = ["checkout", "db", "payment", "inventory", "shipping"]
OPERATIONS = ["query", "authorize", "reserve", "validate", "compute"]


def build_tree(tracer, remaining: list[int], branching: int, depth: int, max_depth: int):
    """Recursively emit a parent/child span tree until `remaining` spans are used."""
    if remaining[0] <= 0 or depth >= max_depth:
        return
    service = random.choice(SERVICES)
    op = random.choice(OPERATIONS)
    with tracer.start_as_current_span(f"{service}.{op}") as span:
        span.set_attribute("service.name", service)
        remaining[0] -= 1
        # Simulate real work so spans have nonzero, varied duration.
        time.sleep(random.uniform(0.0001, 0.002))
        children = random.randint(1, branching) if remaining[0] > 0 else 0
        for _ in range(children):
            if remaining[0] <= 0:
                break
            build_tree(tracer, remaining, branching, depth + 1, max_depth)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spans", type=int, required=True, help="total span count for the trace")
    parser.add_argument("--protocol", choices=["grpc", "http"], default="grpc")
    parser.add_argument("--endpoint", default=None, help="defaults to localhost:4317 (grpc) or http://localhost:4318/v1/traces (http)")
    parser.add_argument("--branching", type=int, default=4, help="max children per span")
    parser.add_argument("--max-depth", type=int, default=12, help="max tree depth, to keep the recursion realistic rather than deeply linear")
    args = parser.parse_args()

    endpoint = args.endpoint or (
        "localhost:4317" if args.protocol == "grpc" else "http://localhost:4318/v1/traces"
    )

    provider = TracerProvider(resource=Resource.create({"service.name": "producer-spike"}))
    if args.protocol == "grpc":
        from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
        exporter = OTLPSpanExporter(endpoint=endpoint, insecure=True)
    else:
        from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
        exporter = OTLPSpanExporter(endpoint=endpoint)

    provider.add_span_processor(BatchSpanProcessor(exporter))
    trace.set_tracer_provider(provider)
    tracer = trace.get_tracer("spec00.producer")

    root_service = random.choice(SERVICES)
    remaining = [args.spans - 1]  # root span counts as one
    t0 = time.time()
    with tracer.start_as_current_span(f"POST /{root_service}") as root:
        root.set_attribute("service.name", root_service)
        root.set_attribute("http.method", "POST")
        children = random.randint(2, args.branching)
        for _ in range(children):
            if remaining[0] <= 0:
                break
            build_tree(tracer, remaining, args.branching, depth=1, max_depth=args.max_depth)
    elapsed = time.time() - t0

    provider.force_flush()
    provider.shutdown()

    emitted = args.spans - remaining[0]
    print(f"emitted {emitted} spans (requested {args.spans}) in {elapsed:.2f}s to {endpoint} ({args.protocol})")


if __name__ == "__main__":
    main()
