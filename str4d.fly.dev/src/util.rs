use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
    routing::{future::RouteFuture, get, MethodRouter, Route},
    Router,
};
use hyper::{header::HOST, StatusCode};
use tower::{Layer, Service};

pub(crate) mod github;

fn req_host(req: &Request) -> Option<&str> {
    // RFC 9112 Section 3.2.2:
    // > When an origin server receives a request with an absolute-form of
    // > request-target, the origin server MUST ignore the received Host header field
    // > (if any) and instead use the host information of the request-target. Note
    // > that if the request-target does not have an authority component, an empty
    // > Host header field will be sent in this case.
    // >
    // > A server MUST accept the absolute-form in requests even though most HTTP/1.1
    // > clients will only send the absolute-form to a proxy.
    req.uri().host().or_else(|| {
        req.headers()
            .get(HOST)
            .expect("Already validated")
            .to_str()
            .ok()
    })
}

/// A multiplexer that enables a single server to serve multiple hosts with independent
/// [`Router`]s.
pub(crate) struct Multiplexer<S> {
    routers: HashMap<&'static str, Router<S>>,
    fallback: Router<S>,
}

impl<S> Clone for Multiplexer<S> {
    fn clone(&self) -> Self {
        Self {
            routers: self.routers.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<S> Multiplexer<S>
where
    S: Clone + Send + Sync + 'static,
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
    ///
    /// The `aliases` are added as temporary (HTTP 307) redirects to the host.
    pub(crate) fn add(
        self,
        host: &'static str,
        aliases: impl IntoIterator<Item = &'static str>,
        router: Router<S>,
    ) -> Self {
        aliases
            .into_iter()
            .fold(self.handle(host, router), |this, alias| {
                this.redirect_temporary(alias, host)
            })
    }

    /// Handles requests for the given host by directing them to the given router.
    pub(crate) fn handle(mut self, host: &'static str, router: Router<S>) -> Self {
        self.routers.insert(host, router);
        self
    }

    /// Adds a temporary (HTTP 307) redirect between two hosts.
    ///
    /// Requests with host `<from>` will be redirected to `https://<to><path_and_query>`.
    pub(crate) fn redirect_temporary(self, from: &'static str, to: &'static str) -> Self {
        self.redirect_inner(from, to, Redirect::temporary)
    }

    /// Adds a redirect between two hosts.
    ///
    /// Requests with host `<from>` will be redirected to `https://<to><path_and_query>`.
    fn redirect_inner<F>(self, from: &'static str, to: &'static str, redirect: F) -> Self
    where
        F: FnOnce(&str) -> Redirect,
        F: Clone + Send + 'static,
    {
        self.handle(
            from,
            Router::new().fallback(move |req: Request| async move {
                let to_uri = format!(
                    "https://{}{}",
                    to,
                    req.uri().path_and_query().map(|p| p.as_str()).unwrap_or(""),
                );
                redirect(&to_uri)
            }),
        )
    }

    /// Applies a [`tower::Layer`] to all routers in the multiplexer.
    ///
    /// This can be used to add additional processing to a request for a group of routers.
    ///
    /// Note that the middleware is only applied to existing routers. So you have to first
    /// add your routers and then call `layer` afterwards. Additional routers added after
    /// `layer` is called will not have the middleware added.
    pub(crate) fn layer<L>(self, layer: L) -> Multiplexer<S>
    where
        L: Layer<Route> + Clone + Send + 'static,
        L::Service: Service<Request> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
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

impl Service<Request> for Multiplexer<()> {
    type Response = Response;
    type Error = Infallible;
    type Future = RouteFuture<Infallible>;

    #[inline]
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `Multiplexer` only wraps `Router`, which is always ready.
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        if let Some(router) = env::var("TEST_HOST")
            .ok()
            .as_deref()
            .or_else(|| req_host(&req))
            .and_then(|s| self.routers.get_mut(s))
        {
            router.call(req)
        } else {
            self.fallback.call(req)
        }
    }
}

#[derive(Clone)]
pub(crate) struct MetricsLayer {}

impl MetricsLayer {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

#[derive(Clone)]
pub(crate) struct MetricsService<S> {
    inner: S,
}

impl<S, RB> Service<Request> for MetricsService<S>
where
    S: Service<Request, Response = Response<RB>> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = <S::Future as Future>::Output> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let host = req_host(&req);
        let handler = format!(
            "{}{}",
            host.unwrap_or_default(),
            req.uri()
                .path_and_query()
                .map(|p| p.as_str())
                .unwrap_or("/")
        );
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let res = inner.call(req).await;
            if let Ok(response) = &res {
                // Only collect metrics for requests we expected to handle.
                let status = response.status();
                if status.is_success() || status.is_redirection() {
                    metrics::counter!("http.requests.total", "handler" => handler).increment(1);
                }
            }
            res
        })
    }
}

pub(crate) fn get_temp_redir<S>(uri: &str) -> MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    let redir = Redirect::temporary(uri);
    get(|| async { redir })
}
