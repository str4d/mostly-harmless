use std::{
    collections::HashMap,
    convert::Infallible,
    task::{Context, Poll},
};

use axum::{
    body::HttpBody,
    response::{IntoResponse, Response},
    routing::{future::RouteFuture, Route},
    Router,
};
use hyper::{header::HOST, Request, StatusCode};
use tower::{Layer, Service};

/// A multiplexer that enables a single server to serve multiple hosts with independent
/// [`Router`]s.
pub(crate) struct Multiplexer<S, B> {
    routers: HashMap<&'static str, Router<S, B>>,
    fallback: Router<S, B>,
}

impl<S, B> Clone for Multiplexer<S, B> {
    fn clone(&self) -> Self {
        Self {
            routers: self.routers.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<S, B> Multiplexer<S, B>
where
    S: Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static,
{
    /// Creates a new `Multiplexer`.
    ///
    /// Unless you add additional routers this will respond with `421 Misdirected Request`
    /// to all requests.
    pub(crate) fn new() -> Self {
        Self {
            routers: HashMap::new(),
            fallback: Router::new().fallback(|| async { StatusCode::MISDIRECTED_REQUEST }),
        }
    }

    /// Handles requests for the given host by directing them to the given router.
    pub(crate) fn handle(mut self, host: &'static str, router: Router<S, B>) -> Self {
        self.routers.insert(host, router);
        self
    }

    /// Applies a [`tower::Layer`] to all routers in the multiplexer.
    ///
    /// This can be used to add additional processing to a request for a group of routers.
    ///
    /// Note that the middleware is only applied to existing routers. So you have to first
    /// add your routers and then call `layer` afterwards. Additional routers added after
    /// `layer` is called will not have the middleware added.
    pub(crate) fn layer<L, NewReqBody>(self, layer: L) -> Multiplexer<S, NewReqBody>
    where
        L: Layer<Route<B>> + Clone + Send + 'static,
        L::Service: Service<Request<NewReqBody>> + Clone + Send + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request<NewReqBody>>>::Future: Send + 'static,
        NewReqBody: HttpBody + 'static,
    {
        let routers = self
            .routers
            .into_iter()
            .map(|(host, router)| (host, router.layer(layer.clone())))
            .collect();

        Multiplexer {
            routers,
            fallback: self.fallback.layer(layer),
        }
    }
}

impl<B> Service<Request<B>> for Multiplexer<(), B>
where
    B: HttpBody + Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<B, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `Multiplexer` only wraps `Router`, which is always ready.
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        // RFC 9112 Section 3.2.2:
        // > When an origin server receives a request with an absolute-form of
        // > request-target, the origin server MUST ignore the received Host header field
        // > (if any) and instead use the host information of the request-target. Note
        // > that if the request-target does not have an authority component, an empty
        // > Host header field will be sent in this case.
        // >
        // > A server MUST accept the absolute-form in requests even though most HTTP/1.1
        // > clients will only send the absolute-form to a proxy.
        let host = req.uri().host().or_else(|| {
            req.headers()
                .get(HOST)
                .expect("Already validated")
                .to_str()
                .ok()
        });

        if let Some(router) = host.and_then(|s| self.routers.get_mut(s)) {
            router.call(req)
        } else {
            self.fallback.call(req)
        }
    }
}
