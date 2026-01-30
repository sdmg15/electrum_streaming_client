use crate::pending_request::{PendingRequest, RequestExt};
use crate::{MaybeBatch, ResponseResult};

/// A builder for batching multiple requests to the Electrum server.
///
/// This type allows queuing both:
/// - tracked requests via [`request`] (which take a callback to receive the typed response), and
/// - event-style requests via [`event_request`] (which emit [`Event`]s through the event receiver
///   instead of a callback).
///
/// After building the batch, submit it using [`AsyncClient::send_batch`] or
/// [`BlockingClient::send_batch`]. The batch will be converted into a raw JSON-RPC message and
/// sent to the server.
///
/// [`request`]: Self::request
/// [`event_request`]: Self::event_request
/// [`AsyncClient::send_batch`]: crate::AsyncClient::send_batch
/// [`BlockingClient::send_batch`]: crate::BlockingClient::send_batch
/// [`Event`]: crate::Event
#[must_use]
#[derive(Debug, Default)]
pub struct BatchRequest {
    inner: Option<MaybeBatch<PendingRequest>>,
}

impl BatchRequest {
    /// Creates a new empty batch request builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes the batch and returns its raw contents, if any requests were added.
    ///
    /// Returns `Some` if the batch is non-empty, or `None` if it was empty.
    pub fn into_inner(self) -> Option<MaybeBatch<PendingRequest>> {
        self.inner
    }

    /// Adds a tracked request to the batch with a typed callback.
    ///
    /// The callback will be invoked with the deserialized response (or error) once the server
    /// replies. The callback is type-erased internally, so it works for both async and blocking
    /// clients.
    pub fn request<Req, F>(&mut self, req: Req, callback: F)
    where
        Req: RequestExt + Send + Sync + 'static,
        F: FnOnce(ResponseResult<Req::Response>) + Send + Sync + 'static,
    {
        MaybeBatch::push_opt(&mut self.inner, PendingRequest::new(req, Some(callback)));
    }

    /// Adds a tracked request and returns an async receiver for the response.
    ///
    /// This is a convenience wrapper around [`request`](Self::request) that creates a
    /// [`futures::channel::oneshot`] channel internally. The returned receiver can be
    /// `.await`ed after the batch is sent.
    pub fn request_async<Req>(
        &mut self,
        req: Req,
    ) -> futures::channel::oneshot::Receiver<ResponseResult<Req::Response>>
    where
        Req: RequestExt + Send + Sync + 'static,
        Req::Response: Send,
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.request(req, move |result| {
            let _ = tx.send(result);
        });
        rx
    }

    /// Adds a tracked request and returns a blocking receiver for the response.
    ///
    /// This is a convenience wrapper around [`request`](Self::request) that creates a
    /// [`std::sync::mpsc::sync_channel`] internally. The returned receiver can be used
    /// with [`recv`](std::sync::mpsc::Receiver::recv) after the batch is sent.
    pub fn request_blocking<Req>(
        &mut self,
        req: Req,
    ) -> std::sync::mpsc::Receiver<ResponseResult<Req::Response>>
    where
        Req: RequestExt + Send + Sync + 'static,
        Req::Response: Send,
    {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.request(req, move |result| {
            let _ = tx.send(result);
        });
        rx
    }

    /// Adds an event-style request to the batch.
    ///
    /// These requests do not take a callback. Any server response (including the initial result
    /// and any future notifications) will be delivered as [`Event`]s through the event receiver.
    ///
    /// Use this for subscription-style RPCs where responses should be handled uniformly as events.
    ///
    /// [`Event`]: crate::Event
    pub fn event_request<Req: RequestExt + Send + Sync + 'static>(&mut self, req: Req) {
        MaybeBatch::push_opt(&mut self.inner, PendingRequest::event(req));
    }
}

/// An error that can occur when sending a request or polling its result.
///
/// This error is returned by client `send_request` methods when the response cannot be obtained.
///
/// It typically indicates that the batch was dropped, the client shut down, or the request
/// failed to be processed internally.
#[derive(Debug)]
pub enum BatchRequestError {
    /// The request was canceled before a response was received.
    ///
    /// This can occur if the client shuts down or if the request is dropped internally.
    Canceled,

    /// The server returned a response error.
    ///
    /// This indicates that the Electrum server replied with an error object, rather than a result.
    Response(crate::ResponseError),
}

impl std::fmt::Display for BatchRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canceled => write!(f, "Request was canceled before being satisfied."),
            Self::Response(e) => write!(f, "Request satisfied with error: {}", e),
        }
    }
}

impl std::error::Error for BatchRequestError {}
