use super::bytes::BytesRWTracker;
use crate::utils::macros::define_inner_service_accessors;
use crate::{
    net::client::{ConnectorService, EstablishedClientConnection},
    stream::Stream,
    Context, Layer, Service,
};
use std::fmt;

/// A [`Service`] that wraps a [`Service`]'s output IO [`Stream`] with an atomic R/W tracker.
///
/// [`Service`]: crate::Service
/// [`Stream`]: crate::stream::Stream
pub struct OutgoingBytesTrackerService<S> {
    inner: S,
}

impl<S: fmt::Debug> fmt::Debug for OutgoingBytesTrackerService<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutgoingBytesTrackerService")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S> OutgoingBytesTrackerService<S> {
    /// Create a new [`OutgoingBytesTrackerService`].
    ///
    /// See [`OutgoingBytesTrackerService`] for more information.
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }

    define_inner_service_accessors!();
}

impl<S> Clone for OutgoingBytesTrackerService<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S, State, Request> Service<State, Request> for OutgoingBytesTrackerService<S>
where
    S: ConnectorService<State, Request, Connection: Stream + Unpin, Error: Send + Sync + 'static>,
    State: Send + Sync + 'static,
    Request: Send + 'static,
{
    type Response = EstablishedClientConnection<BytesRWTracker<S::Connection>, State, Request>;
    type Error = S::Error;

    async fn serve(
        &self,
        ctx: Context<State>,
        req: Request,
    ) -> Result<Self::Response, Self::Error> {
        let EstablishedClientConnection {
            mut ctx,
            req,
            conn,
            addr,
        } = self.inner.connect(ctx, req).await?;
        let conn = BytesRWTracker::new(conn);
        let handle = conn.handle();
        ctx.insert(handle);
        Ok(EstablishedClientConnection {
            ctx,
            req,
            conn,
            addr,
        })
    }
}

/// A [`Layer`] that wraps a [`Service`]'s output IO [`Stream`] with an atomic R/W tracker.
///
/// [`Layer`]: crate::Layer
/// [`Service`]: crate::Service
/// [`Stream`]: crate::stream::Stream
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OutgoingBytesTrackerLayer;

impl OutgoingBytesTrackerLayer {
    /// Create a new [`OutgoingBytesTrackerLayer`].
    pub const fn new() -> Self {
        Self
    }
}

impl Default for OutgoingBytesTrackerLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for OutgoingBytesTrackerLayer {
    type Service = OutgoingBytesTrackerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OutgoingBytesTrackerService { inner }
    }
}