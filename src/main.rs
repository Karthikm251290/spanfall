use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;
use parking_lot::RwLock;
use spanfall::api;
use spanfall::api::ApiState;
use spanfall::ingest::grpc::GrpcTraceService;
use spanfall::ingest::handler::IngestState;
use spanfall::ingest::http;
use spanfall::ingest::receiver_state::ReceiverState;
use spanfall::store::writer::spawn_writer;
use spanfall::store::Store;

// ponytail: hardcoded defaults and fixed ports -- the full CLI surface (--max-memory,
// --ingest-timeout, subcommands, --dump, port-conflict naming) belongs to spec 07
// (`docs/specs/07-cli-demo-docs.md`, which owns `src/main.rs`/`src/cli.rs`). This is just enough
// wiring to prove spec 01's listeners end-to-end; spec 07 replaces it with real flag parsing.
const MAX_MEMORY_BYTES: usize = 1 << 30; // 1 GiB, STOP #5
const INGEST_TIMEOUT: Duration = Duration::from_secs(10);
const GRPC_ADDR: &str = "127.0.0.1:4317";
const HTTP_ADDR: &str = "127.0.0.1:4318";
const API_ADDR: &str = "127.0.0.1:5317"; // STOP #2

#[tokio::main]
async fn main() {
    println!("{} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
    println!("OTLP gRPC: 4317   OTLP HTTP: 4318   Read API: 5317");

    let store = Arc::new(RwLock::new(Store::new(MAX_MEMORY_BYTES)));
    let (tx, events_seq, _writer_handle) = spawn_writer(store.clone(), 1024);
    let receiver = Arc::new(RwLock::new(ReceiverState::default()));
    let state = IngestState::new(tx, receiver.clone(), INGEST_TIMEOUT);
    let api_state = ApiState { store, receiver, events_seq };

    let grpc_addr: SocketAddr = GRPC_ADDR.parse().expect("valid hardcoded address");
    let http_addr: SocketAddr = HTTP_ADDR.parse().expect("valid hardcoded address");
    let api_addr: SocketAddr = API_ADDR.parse().expect("valid hardcoded address");

    let grpc = tonic::transport::Server::builder()
        .add_service(TraceServiceServer::new(GrpcTraceService::new(state.clone())))
        .serve(grpc_addr);

    let http_listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .unwrap_or_else(|e| panic!("bind {HTTP_ADDR}: {e}"));
    let http = axum::serve(http_listener, http::router(state));

    let api_listener =
        tokio::net::TcpListener::bind(api_addr).await.unwrap_or_else(|e| panic!("bind {API_ADDR}: {e}"));
    let api = axum::serve(api_listener, api::router(api_state));

    tokio::select! {
        res = grpc => if let Err(e) = res { eprintln!("gRPC server error: {e}"); },
        res = http => if let Err(e) = res { eprintln!("HTTP server error: {e}"); },
        res = api => if let Err(e) = res { eprintln!("API server error: {e}"); },
        _ = tokio::signal::ctrl_c() => println!("shutting down"),
    }
}
