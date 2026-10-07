//! The local hook server: `POST /events` on 127.0.0.1 (a random port, never another
//! interface), with a per-run token in the `x-foundry-token` header.
//!
//! Each request is checked (token, size, JSON, known project, valid agent id) and answered
//! at once with `202`; the events are worked out afterwards, in arrival order, on one worker
//! thread. Refusals answer `{ code, message }` with these codes (the helper's interface, not
//! the Tauri contract): `unauthorized` (401), `payload_too_large` (413), `bad_request` (400),
//! `unknown_project` (404), `not_found` (404, any other route), `method_not_allowed` (405),
//! `busy` (503, the queue is full).

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};

use super::{AgentEventsStore, HookPost};
use crate::core::hook::{HookEndpoint, HookEndpointCell, TOKEN_HEADER};
use crate::core::{ids, AppError, ErrorCode};

/// The largest body accepted.
pub const BODY_CAP: usize = 256 * 1024;
/// Hook calls waiting to be classified; beyond this, `busy`.
const QUEUE: usize = 1024;
const HEADER_TIMEOUT: Duration = Duration::from_secs(5);
pub const ROUTE: &str = "/events";

/// The running server. Dropping it stops accepting connections and withdraws the endpoint.
pub struct HookServer {
    address: SocketAddr,
    endpoint: HookEndpoint,
    published: Arc<HookEndpointCell>,
    accept: tauri::async_runtime::JoinHandle<()>,
}

impl HookServer {
    pub fn endpoint(&self) -> &HookEndpoint {
        &self.endpoint
    }

    /// The bound address: always 127.0.0.1.
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for HookServer {
    fn drop(&mut self) {
        self.accept.abort();
        self.published.clear();
    }
}

/// 256 bits from two v4 UUIDs, as 64 hex digits.
fn new_token() -> String {
    format!("{}{}", ids::new_id(), ids::new_id()).replace('-', "")
}

/// Binds 127.0.0.1 on a random port, publishes the port and a fresh token to `published`,
/// and starts serving.
pub fn start(
    store: Arc<AgentEventsStore>,
    published: Arc<HookEndpointCell>,
) -> Result<HookServer, AppError> {
    let listener = StdListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let endpoint = HookEndpoint {
        port: address.port(),
        token: new_token(),
    };

    let (queue, inbox) = mpsc::sync_channel::<HookPost>(QUEUE);
    let worker_store = Arc::clone(&store);
    std::thread::Builder::new()
        .name("foundry-hook-events".into())
        .spawn(move || {
            // Ends once the server and every connection have dropped their senders.
            for post in inbox {
                if let Err(err) = worker_store.ingest(&post) {
                    tracing::warn!(agent_id = %post.agent_id, error = %err, "hook call not stored");
                }
            }
        })?;

    let ctx = Arc::new(Ctx {
        store,
        token: endpoint.token.clone(),
        queue,
    });
    let accept = tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(error = %err, "hook server could not start");
                return;
            }
        };
        loop {
            let stream = match listener.accept().await {
                Ok((stream, _)) => stream,
                Err(err) => {
                    tracing::warn!(error = %err, "hook server accept failed");
                    continue;
                }
            };
            let ctx = Arc::clone(&ctx);
            tauri::async_runtime::spawn(async move {
                let service = hyper::service::service_fn(move |req| {
                    let ctx = Arc::clone(&ctx);
                    async move { Ok::<_, Infallible>(handle(req, &ctx).await) }
                });
                let served = hyper::server::conn::http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(HEADER_TIMEOUT)
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
                if let Err(err) = served {
                    tracing::debug!(error = %err, "hook connection ended with an error");
                }
            });
        }
    });

    published.publish(endpoint.clone());
    tracing::info!(port = endpoint.port, "hook server listening on 127.0.0.1");
    Ok(HookServer {
        address,
        endpoint,
        published,
        accept,
    })
}

struct Ctx {
    store: Arc<AgentEventsStore>,
    token: String,
    queue: SyncSender<HookPost>,
}

fn reply(status: StatusCode, code: &str, message: &str) -> Response<Full<Bytes>> {
    let body = serde_json::json!({ "code": code, "message": message }).to_string();
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json"),
    );
    response
}

/// Compares in time independent of where the first difference is.
fn same_token(given: &[u8], expected: &[u8]) -> bool {
    given.len() == expected.len()
        && given
            .iter()
            .zip(expected)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

async fn handle(req: Request<Incoming>, ctx: &Ctx) -> Response<Full<Bytes>> {
    if req.uri().path() != ROUTE {
        return reply(StatusCode::NOT_FOUND, "not_found", "no such route");
    }
    if req.method() != Method::POST {
        return reply(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "use POST",
        );
    }
    let token_ok = req
        .headers()
        .get(TOKEN_HEADER)
        .is_some_and(|value| same_token(value.as_bytes(), ctx.token.as_bytes()));
    if !token_ok {
        return reply(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or wrong token",
        );
    }
    let too_large = || {
        reply(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "the body is over 256 KB",
        )
    };
    let declared = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|len| len > BODY_CAP as u64) {
        return too_large();
    }
    let body = match Limited::new(req.into_body(), BODY_CAP).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) if err.is::<http_body_util::LengthLimitError>() => return too_large(),
        Err(_) => return reply(StatusCode::BAD_REQUEST, "bad_request", "unreadable body"),
    };
    let post: HookPost = match serde_json::from_slice(&body) {
        Ok(post) => post,
        Err(err) => {
            return reply(
                StatusCode::BAD_REQUEST,
                "bad_request",
                &format!("not a hook body: {err}"),
            )
        }
    };
    if let Err(err) = ctx.store.check(&post.project_id, &post.agent_id) {
        return match err.code {
            ErrorCode::NotFound | ErrorCode::FolderMissing => {
                reply(StatusCode::NOT_FOUND, "unknown_project", &err.message)
            }
            _ => reply(StatusCode::BAD_REQUEST, "bad_request", &err.message),
        };
    }
    match ctx.queue.try_send(post) {
        Ok(()) => {
            let mut response = Response::new(Full::new(Bytes::from_static(b"{\"accepted\":true}")));
            *response.status_mut() = StatusCode::ACCEPTED;
            response
        }
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => reply(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "too many hook calls waiting",
        ),
    }
}
