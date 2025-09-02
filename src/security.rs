use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, http::header};
use futures_util::future::{LocalBoxFuture, ready, Ready};
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct SecurityHeaders {
    pub enable_hsts: bool,
}

impl SecurityHeaders {
    pub fn from_env() -> Self {
        let enable_hsts = std::env::var("ENABLE_HSTS").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        Self { enable_hsts }
    }

    pub fn with_hsts(mut self, enable: bool) -> Self {
        self.enable_hsts = enable;
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SecurityHeadersMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddleware {
            service: Rc::new(service),
            cfg: self.clone(),
        }))
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: Rc<S>,
    cfg: SecurityHeaders,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let cfg = self.cfg.clone();
        Box::pin(async move {
            let mut res = svc.call(req).await?;
            let headers = res.response_mut().headers_mut();
            if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
                headers.insert(header::CONTENT_SECURITY_POLICY, header::HeaderValue::from_static("default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"));
            }
            if !headers.contains_key(header::REFERRER_POLICY) {
                headers.insert(header::REFERRER_POLICY, header::HeaderValue::from_static("no-referrer"));
            }
            if !headers.contains_key(header::X_CONTENT_TYPE_OPTIONS) {
                headers.insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));
            }
            if !headers.contains_key(header::X_FRAME_OPTIONS) {
                headers.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("DENY"));
            }
            if !headers.contains_key(header::X_XSS_PROTECTION) {
                headers.insert(header::X_XSS_PROTECTION, header::HeaderValue::from_static("0"));
            }
            if cfg.enable_hsts && !headers.contains_key(header::STRICT_TRANSPORT_SECURITY) {
                headers.insert(header::STRICT_TRANSPORT_SECURITY, header::HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"));
            }
            Ok(res)
        })
    }
}
