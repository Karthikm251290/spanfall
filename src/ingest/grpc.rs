use opentelemetry_proto::tonic::collector::trace::v1::{
    trace_service_server::TraceService, ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use tonic::{Request, Response, Status};

use super::convert::RejectReason;
use super::handler::{IngestError, IngestState};
use super::receiver_state::Transport;

/// OTLP/gRPC on :4317 (§3). Decoding happens in tonic itself before `export()` runs; this handler
/// only owns the shared accept-or-reject decision and its wire-error mapping.
pub struct GrpcTraceService {
    state: IngestState,
}

impl GrpcTraceService {
    pub fn new(state: IngestState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl TraceService for GrpcTraceService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        match self.state.accept(request.into_inner(), Transport::GrpcV4317).await {
            Ok(()) => Ok(Response::new(ExportTraceServiceResponse::default())),
            // EmptyPayload (§2) is a named, expected condition, not a wire failure — the
            // exporter still gets a normal OK; the reject surfaces via the Receiver ring buffer.
            Err(IngestError::Reject(RejectReason::EmptyPayload)) => {
                Ok(Response::new(ExportTraceServiceResponse::default()))
            }
            Err(IngestError::Reject(reason)) => Err(Status::invalid_argument(format!("{reason:?}"))),
            Err(IngestError::ConversionPanicked) => Err(Status::internal("conversion panicked")),
            Err(IngestError::Backpressure) => Err(Status::resource_exhausted(
                "store write backlog exceeded --ingest-timeout, retry",
            )),
            Err(IngestError::WriterGone) => Err(Status::internal("writer task not running")),
        }
    }
}
