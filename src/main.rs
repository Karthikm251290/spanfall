use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use parking_lot::RwLock;
use spanfall::api;
use spanfall::api::ApiState;
use spanfall::cli::{Cli, Command};
use spanfall::demo;
use spanfall::dump;
use spanfall::ingest::handler::IngestState;
use spanfall::ingest::http;
use spanfall::ingest::receiver_state::ReceiverState;
use spanfall::ingest::sniff::serve_grpc_with_http_sniffing;
use spanfall::port_conflict;
use spanfall::store::writer::spawn_writer;
use spanfall::store::Store;

const GRPC_ADDR: &str = "127.0.0.1:4317";
const HTTP_ADDR: &str = "127.0.0.1:4318";
const API_ADDR: &str = "127.0.0.1:5317"; // STOP #2

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.dump && cli.command.is_some() {
        let name = env!("CARGO_BIN_NAME");
        eprintln!("{name}: --dump does not take a subcommand (did you mean `{name} list`?)");
        std::process::exit(2);
    }
    if cli.demo && (cli.dump || cli.command.is_some()) {
        eprintln!(
            "{}: --demo seeds a new server, it doesn't query one -- drop --dump/list",
            env!("CARGO_BIN_NAME")
        );
        std::process::exit(2);
    }

    if cli.dump {
        exit_on_err(dump::print_dump(API_ADDR));
        return;
    }
    if let Some(Command::List) = cli.command {
        exit_on_err(dump::print_list(API_ADDR));
        return;
    }

    run_server(cli.max_memory, Duration::from_secs(cli.ingest_timeout), cli.demo).await;
}

fn exit_on_err(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("{}: {e}", env!("CARGO_BIN_NAME"));
        std::process::exit(1);
    }
}

async fn run_server(max_memory_bytes: usize, ingest_timeout: Duration, seed_demo: bool) {
    let grpc_addr: SocketAddr = GRPC_ADDR.parse().expect("valid hardcoded address");
    let http_addr: SocketAddr = HTTP_ADDR.parse().expect("valid hardcoded address");
    let api_addr: SocketAddr = API_ADDR.parse().expect("valid hardcoded address");

    // Bind every listener before printing anything -- a port conflict on any of the three must
    // not print a "Listening" banner that isn't true yet.
    let grpc_listener = bind_or_exit(grpc_addr).await;
    let http_listener = bind_or_exit(http_addr).await;
    let api_listener = bind_or_exit(api_addr).await;

    println!("Listening on http://localhost:5317");
    println!("OTLP gRPC: 4317   OTLP HTTP: 4318");
    if seed_demo {
        println!("--demo: pre-seeded with a curated fake trace set");
    }

    let mut store = Store::new(max_memory_bytes);
    if seed_demo {
        demo::seed(&mut store);
    }
    let store = Arc::new(RwLock::new(store));
    let (tx, events_seq, _writer_handle) = spawn_writer(store.clone(), 1024);
    let receiver = Arc::new(RwLock::new(ReceiverState::default()));
    let state = IngestState::new(tx, receiver.clone(), ingest_timeout);
    let api_state = ApiState { store, receiver, events_seq };

    let grpc = serve_grpc_with_http_sniffing(grpc_listener, state.clone());
    let http = axum::serve(http_listener, http::router(state));
    let api = axum::serve(api_listener, api::router(api_state));

    tokio::select! {
        res = grpc => if let Err(e) = res { eprintln!("gRPC server error: {e}"); },
        res = http => if let Err(e) = res { eprintln!("HTTP server error: {e}"); },
        res = api => if let Err(e) = res { eprintln!("API server error: {e}"); },
        _ = tokio::signal::ctrl_c() => println!("shutting down"),
    }
}

/// T16: on `AddrInUse`, name the process holding the port instead of printing the raw OS error.
async fn bind_or_exit(addr: SocketAddr) -> tokio::net::TcpListener {
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let holder = port_conflict::describe_holder(addr.port())
                .map(|h| format!(" -- held by {h}"))
                .unwrap_or_default();
            eprintln!("{}: port {} is already in use{holder}", env!("CARGO_BIN_NAME"), addr.port());
            std::process::exit(1);
        }
        Err(e) => panic!("bind {addr}: {e}"),
    }
}
