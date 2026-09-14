use std::io;
use std::time::Duration;

use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::TraceServiceServer;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;

use super::grpc::GrpcTraceService;
use super::handler::IngestState;

const MISDIRECT_BODY: &str =
    "This is the OTLP/gRPC port (4317). Point your OTLP/HTTP exporter at :4318 instead.";

/// Accepts on :4317, `peek()`s each connection's first bytes non-destructively, and forks: a real
/// gRPC client's h2 connection preface goes to tonic unchanged; anything else -- most commonly an
/// OTLP/HTTP exporter misconfigured onto 4317, the single most documented OTel first-run confusion
/// -- gets a canned 400 naming the fix, written straight to the socket and readable in the
/// exporter's own log with the UI never opened (§3, acceptance #2).
pub async fn serve_grpc_with_http_sniffing(
    listener: TcpListener, state: IngestState,
) -> Result<(), tonic::transport::Error> {
    let (tx, rx) = mpsc::channel::<io::Result<TcpStream>>(16);

    tokio::spawn(async move {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(pair) => pair,
                Err(e) => {
                    if tx.send(Err(e)).await.is_err() {
                        return;
                    }
                    continue;
                }
            };
            if looks_like_h2_preface(&stream).await {
                if tx.send(Ok(stream)).await.is_err() {
                    return;
                }
            } else {
                tokio::spawn(reject_with_400(stream));
            }
        }
    });

    Server::builder()
        .add_service(TraceServiceServer::new(GrpcTraceService::new(state)))
        .serve_with_incoming(ReceiverStream::new(rx))
        .await
}

/// The h2 client connection preface starts `PRI * HTTP/2.0\r\n\r\n...`; an HTTP/1.1 request line
/// (any method) never does. Four bytes distinguish them. `peek()` leaves the bytes in the socket
/// buffer so tonic still sees the full connection. A connection that never delivers 4 bytes within
/// a few polls falls to the 400 path -- failing toward the informative reject, not toward tonic.
///
/// `peek()`'s readiness is level-triggered: once *any* bytes have arrived, awaiting it again
/// returns immediately with the same short read instead of waiting for the rest of the preface,
/// so retrying with no delay would burn all five attempts before a slow-arriving preface finishes.
/// The `sleep` between attempts is what actually buys the sender time to deliver the rest.
async fn looks_like_h2_preface(stream: &TcpStream) -> bool {
    let mut buf = [0u8; 4];
    for attempt in 0..5 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        match stream.peek(&mut buf).await {
            Ok(4) => return &buf == b"PRI ",
            Ok(0) => return false, // peer closed without sending anything
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
    false
}

async fn reject_with_400(mut stream: TcpStream) {
    // Drain whatever the client already sent. `peek()` never consumes it, and closing a socket
    // with unread bytes still queued makes the OS send an RST instead of a clean FIN -- which
    // can cut off the response we're about to write instead of just the (unread) request body.
    // `try_read` never blocks, so this only drains what's already buffered, never waits for more.
    let mut discard = [0u8; 4096];
    loop {
        match stream.try_read(&mut discard) {
            Ok(0) | Err(_) => break,
            Ok(_) => continue,
        }
    }

    let response = format!(
        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        MISDIRECT_BODY.len(),
        MISDIRECT_BODY
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use opentelemetry_proto::tonic::collector::trace::v1::trace_service_client::TraceServiceClient;
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status as PbStatus};
    use parking_lot::RwLock;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    use super::*;
    use crate::ingest::receiver_state::ReceiverState;

    fn one_span_request() -> ExportTraceServiceRequest {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: None,
                scope_spans: vec![ScopeSpans {
                    scope: None,
                    spans: vec![Span {
                        trace_id: vec![1; 16],
                        span_id: vec![2; 8],
                        trace_state: String::new(),
                        parent_span_id: Vec::new(),
                        flags: 0,
                        name: "op".to_string(),
                        kind: 0,
                        start_time_unix_nano: 1,
                        end_time_unix_nano: 2,
                        attributes: Vec::new(),
                        dropped_attributes_count: 0,
                        events: Vec::new(),
                        dropped_events_count: 0,
                        links: Vec::new(),
                        dropped_links_count: 0,
                        status: Some(PbStatus { message: String::new(), code: 0 }),
                    }],
                    schema_url: String::new(),
                }],
                schema_url: String::new(),
            }],
        }
    }

    async fn spawn_sniffing_listener() -> (std::net::SocketAddr, mpsc::Receiver<crate::ingest::convert::SpanBatch>)
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel(8);
        let receiver = Arc::new(RwLock::new(ReceiverState::default()));
        let state = IngestState::new(tx, receiver, Duration::from_secs(1));
        tokio::spawn(serve_grpc_with_http_sniffing(listener, state));
        (addr, rx)
    }

    #[tokio::test]
    async fn otlp_http_sent_to_the_grpc_port_gets_a_400_naming_the_fix() {
        let (addr, _rx) = spawn_sniffing_listener().await;

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"POST /v1/traces HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let response = String::from_utf8(buf).unwrap();

        assert!(response.starts_with("HTTP/1.1 400"), "response was: {response}");
        assert!(response.contains("4318"), "response body must name the fix: {response}");
    }

    #[tokio::test]
    async fn a_real_grpc_client_still_reaches_the_writer_through_the_sniffing_listener() {
        let (addr, mut rx) = spawn_sniffing_listener().await;

        let channel = tonic::transport::Endpoint::from_shared(format!("http://{addr}"))
            .unwrap()
            .connect()
            .await
            .unwrap();
        let mut client = TraceServiceClient::new(channel);
        client.export(one_span_request()).await.unwrap();

        // confirms the request made it through the sniffing accept loop into tonic's own
        // handler and all the way to the writer, not just that the TCP connection was accepted.
        let batch = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
        assert_eq!(batch.spans.len(), 1);
    }
}
