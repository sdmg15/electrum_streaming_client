#![doc = include_str!("../README.md")]
mod client;
pub use client::*;
mod custom_serde;
mod hash_types;
pub mod io;
pub mod notification;
mod pending_request;
pub mod protocol;
pub mod request;
mod request_tracker;
pub mod response;
pub use hash_types::*;
pub use pending_request::*;
pub use protocol::*;
pub use request::Request;
pub use request_tracker::*;
pub use serde_json;

/// An owned or borrowed static string.
pub type CowStr = std::borrow::Cow<'static, str>;

/// A double SHA256 hash (`sha256d`) used for Merkle branches and header proofs.
pub type DoubleSHA = bitcoin::hashes::sha256d::Hash;

/// A method name and its corresponding parameters, as used in a JSON-RPC request.
pub type MethodAndParams = (CowStr, Vec<serde_json::Value>);

/// A server response that is either a success (`Ok`) or a JSON-RPC error (`Err`).
pub type ResponseResult<Resp> = Result<Resp, ResponseError>;

/// Internal type aliases for asynchronous client components.
mod async_aliases {
    use super::*;
    use futures::channel::mpsc::{TrySendError, UnboundedReceiver, UnboundedSender};
    use pending_request::PendingRequest;

    /// The sending half of the channel used to enqueue one or more requests from [`AsyncClient`].
    ///
    /// These requests are processed and forwarded to [`RequestTracker::track_request`] to be assigned an ID and serialized.
    pub type AsyncRequestSender = UnboundedSender<RawOneOrMany<PendingRequest>>;

    /// The receiving half of the request channel used internally by the async client.
    ///
    /// Requests sent by [`AsyncClient`] are dequeued here and forwarded to [`RequestTracker::track_request`].
    pub type AsyncRequestReceiver = UnboundedReceiver<RawOneOrMany<PendingRequest>>;

    /// The error returned by [`AsyncClient::send_request`] when a request fails.
    ///
    /// This may occur if the server responds with an error, the request is canceled, or the client is shut down.
    pub type AsyncRequestError = request::Error<AsyncRequestSendError>;

    /// The error that occurs when a request cannot be sent into the async request channel.
    ///
    /// This typically means the client's background task has shut down or the queue is disconnected.
    pub type AsyncRequestSendError = TrySendError<RawOneOrMany<PendingRequest>>;

    /// The sending half of the internal event stream, used to emit [`Event`]s from the client worker loop.
    pub type AsyncEventSender = UnboundedSender<Event>;

    /// The receiving half of the internal event stream, returned to users of [`AsyncClient`].
    ///
    /// This yields all incoming [`Event`]s from the Electrum server, including notifications and responses.
    pub type AsyncEventReceiver = UnboundedReceiver<Event>;
}
pub use async_aliases::*;

/// Internal type aliases for blocking client components.
mod blocking_aliases {
    use super::*;
    use pending_request::PendingRequest;
    use std::sync::mpsc::{Receiver, SendError, Sender};

    /// Channel sender for sending blocking requests from [`BlockingClient`] to the write thread.
    pub type BlockingRequestSender = Sender<RawOneOrMany<PendingRequest>>;

    /// Channel receiver used by the write thread to dequeue pending requests.
    pub type BlockingRequestReceiver = Receiver<RawOneOrMany<PendingRequest>>;

    /// Error returned by [`BlockingClient::send_request`] if the request fails or is canceled.
    pub type BlockingRequestError = request::Error<BlockingRequestSendError>;

    /// Error that occurs when a blocking request cannot be sent to the internal request channel.
    ///
    /// Typically indicates that the client has been shut down.
    pub type BlockingRequestSendError = SendError<RawOneOrMany<PendingRequest>>;

    /// Channel sender used by the read thread to emit [`Event`]s.
    pub type BlockingEventSender = Sender<Event>;

    /// Channel receiver used to receive [`Event`]s from the Electrum server.
    pub type BlockingEventReceiver = Receiver<Event>;
}
pub use blocking_aliases::*;
