use crate::{Event, MethodAndParams, Request, ResponseError, ResponseResult};
use serde_json::Value;

/// Extension trait for request types that can construct [`SatisfiedRequest`] and [`ErroredRequest`].
///
/// This trait is automatically implemented for all built-in request types via the
/// [`gen_pending_request_types!`] macro. It bridges a typed request to the enum variants used in
/// [`Event`].
///
/// [`Event`]: crate::Event
pub trait RequestExt: Request + Sized {
    /// Wraps this request and its decoded response into a [`SatisfiedRequest`].
    fn into_satisfied(self, resp: Self::Response) -> SatisfiedRequest;

    /// Wraps this request and an error into an [`ErroredRequest`].
    fn into_errored(self, error: ResponseError) -> ErroredRequest;
}

macro_rules! gen_pending_request_types {
    ($($name:ident),*) => {
        /// A successfully handled request and its decoded server response.
        ///
        /// This enum is returned when a request has been fully processed and the server replied
        /// with a valid `result`. It contains both the original request and the corresponding
        /// response.
        ///
        /// `SatisfiedRequest` is used by the [`Event::Response`] variant to expose typed
        /// request-response pairs to the caller.
        ///
        /// You typically don't construct this manually — it is created internally by the client
        /// after decoding JSON-RPC responses.
        ///
        /// [`Event::Response`]: crate::Event::Response
        #[derive(Debug, Clone)]
        pub enum SatisfiedRequest {
            $($name {
                req: crate::request::$name,
                resp: <crate::request::$name as Request>::Response,
            }),*,
        }

        /// A request that received an error response from the Electrum server.
        ///
        /// This enum represents a completed request where the server returned a JSON-RPC error
        /// instead of a `result`. It contains both the original request and the associated error.
        ///
        /// This is used by the [`Event::ResponseError`] variant to expose server-side failures
        /// in a typed manner.
        ///
        /// Like [`SatisfiedRequest`], this is created internally by the client during response
        /// processing.
        ///
        /// [`Event::ResponseError`]: crate::Event::ResponseError
        #[derive(Debug, Clone)]
        pub enum ErroredRequest {
            $($name {
                req: crate::request::$name,
                error: ResponseError,
            }),*,
        }

        impl core::fmt::Display for ErroredRequest {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$name { req, error } => write!(f, "Server responsed to {:?} with error: {}", req, error)),*,
                }
            }
        }

        impl std::error::Error for ErroredRequest {}

        $(
            impl RequestExt for crate::request::$name {
                fn into_satisfied(self, resp: <Self as Request>::Response) -> SatisfiedRequest {
                    SatisfiedRequest::$name { req: self, resp }
                }
                fn into_errored(self, error: ResponseError) -> ErroredRequest {
                    ErroredRequest::$name { req: self, error }
                }
            }
        )*
    };
}

gen_pending_request_types! {
    Header,
    HeaderWithProof,
    Headers,
    HeadersWithCheckpoint,
    EstimateFee,
    HeadersSubscribe,
    RelayFee,
    GetBalance,
    GetHistory,
    GetMempool,
    ListUnspent,
    ScriptHashSubscribe,
    ScriptHashUnsubscribe,
    BroadcastTx,
    GetTx,
    GetTxMerkle,
    GetTxidFromPos,
    GetFeeHistogram,
    Banner,
    Ping,
    Custom
}

/// A pending request that has been sent to the Electrum server and is awaiting a response.
///
/// This struct holds a type-erased handler closure that knows how to deserialize the server's
/// raw JSON response and either:
/// - dispatch it through a callback (for tracked requests), or
/// - construct an [`Event`] (for event-style requests).
///
/// Construct via [`PendingRequest::new`] (with a callback) or [`PendingRequest::event`] (without).
///
/// [`Event`]: crate::Event
pub struct PendingRequest {
    method_and_params: MethodAndParams,
    handler: Box<
        dyn FnOnce(Result<Value, Value>) -> Result<Option<Event>, serde_json::Error> + Send + Sync,
    >,
}

impl PendingRequest {
    /// Creates a new pending request with an optional typed callback.
    ///
    /// If `callback` is `Some`, the response will be deserialized and dispatched through it,
    /// and [`State::process_incoming`] will return `Ok(None)` for this request.
    ///
    /// If `callback` is `None`, the response will be wrapped in an [`Event`] and returned from
    /// [`State::process_incoming`].
    ///
    /// [`State::process_incoming`]: crate::State::process_incoming
    /// [`Event`]: crate::Event
    pub fn new<Req: RequestExt + Send + Sync + 'static>(
        req: Req,
        callback: Option<impl FnOnce(ResponseResult<Req::Response>) + Send + Sync + 'static>,
    ) -> Self {
        let method_and_params = req.to_method_and_params();
        Self {
            method_and_params,
            handler: Box::new(move |raw_result| match (raw_result, callback) {
                (Ok(raw_resp), Some(cb)) => {
                    let resp = serde_json::from_value(raw_resp)?;
                    cb(Ok(resp));
                    Ok(None)
                }
                (Ok(raw_resp), None) => {
                    let resp = serde_json::from_value(raw_resp)?;
                    Ok(Some(Event::Response(req.into_satisfied(resp))))
                }
                (Err(raw_err), Some(cb)) => {
                    cb(Err(ResponseError(raw_err)));
                    Ok(None)
                }
                (Err(raw_err), None) => Ok(Some(Event::ResponseError(
                    req.into_errored(ResponseError(raw_err)),
                ))),
            }),
        }
    }

    /// Creates a new pending request without a callback (event-style).
    ///
    /// The server's response will be returned as an [`Event`] from [`State::process_incoming`].
    ///
    /// [`State::process_incoming`]: crate::State::process_incoming
    /// [`Event`]: crate::Event
    pub fn event<Req: RequestExt + Send + Sync + 'static>(req: Req) -> Self {
        Self::new(req, None::<fn(ResponseResult<Req::Response>)>)
    }

    /// Returns the method name and parameters for this request.
    ///
    /// Used to serialize the request into a JSON-RPC message and to reconstruct
    /// [`RawRequest`]s for pending requests.
    ///
    /// [`RawRequest`]: crate::RawRequest
    pub fn to_method_and_params(&self) -> MethodAndParams {
        self.method_and_params.clone()
    }

    /// Handles the raw server result by invoking the type-erased handler closure.
    ///
    /// Returns `Ok(Some(Event))` for event-style requests, `Ok(None)` for callback-dispatched
    /// requests, or `Err` if deserialization failed.
    pub fn handle(self, result: Result<Value, Value>) -> Result<Option<Event>, serde_json::Error> {
        (self.handler)(result)
    }
}

impl std::fmt::Debug for PendingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingRequest")
            .field("method_and_params", &self.method_and_params)
            .finish_non_exhaustive()
    }
}
