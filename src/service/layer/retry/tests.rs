use super::*;
use crate::service::ServiceBuilder;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[tokio::test]
async fn retry_errors() {
    struct Svc {
        errored: AtomicBool,
        response_counter: Arc<AtomicUsize>,
        error_counter: Arc<AtomicUsize>,
    }

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            assert_eq!(req, "hello");
            if self.errored.swap(true, Ordering::SeqCst) {
                self.response_counter.fetch_add(1, Ordering::SeqCst);
                Ok("world")
            } else {
                self.error_counter.fetch_add(1, Ordering::SeqCst);
                Err(Error::from("retry me"))
            }
        }
    }

    let response_counter = Arc::new(AtomicUsize::new(0));
    let error_counter = Arc::new(AtomicUsize::new(0));

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(RetryErrors))
        .service(Svc {
            errored: AtomicBool::new(false),
            response_counter: response_counter.clone(),
            error_counter: error_counter.clone(),
        });

    let resp = svc.serve(Context::default(), "hello").await.unwrap();
    assert_eq!(resp, "world");
    assert_eq!(response_counter.load(Ordering::SeqCst), 1);
    assert_eq!(error_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retry_limit() {
    struct Svc {
        error_counter: Arc<AtomicUsize>,
    }

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            assert_eq!(req, "hello");
            self.error_counter.fetch_add(1, Ordering::SeqCst);
            Err(Error::from("error forever"))
        }
    }

    let error_counter = Arc::new(AtomicUsize::new(0));

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(Limit(Arc::new(Mutex::new(2)))))
        .service(Svc {
            error_counter: error_counter.clone(),
        });

    let err = svc.serve(Context::default(), "hello").await.unwrap_err();
    assert_eq!(err.to_string(), "error forever");
    assert_eq!(error_counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn retry_error_inspection() {
    struct Svc {
        errored: AtomicBool,
    }

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            assert_eq!(req, "hello");
            if self.errored.swap(true, Ordering::SeqCst) {
                Err(Error::from("reject"))
            } else {
                Err(Error::from("retry me"))
            }
        }
    }

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(UnlessErr("reject")))
        .service(Svc {
            errored: AtomicBool::new(false),
        });

    let err = svc.serve(Context::default(), "hello").await.unwrap_err();
    assert_eq!(err.to_string(), "reject");
}

#[tokio::test]
async fn retry_cannot_clone_request() {
    struct Svc;

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            assert_eq!(req, "hello");
            Err(Error::from("failed"))
        }
    }

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(CannotClone))
        .service(Svc);

    let err = svc.serve(Context::default(), "hello").await.unwrap_err();
    assert_eq!(err.to_string(), "failed");
}

#[tokio::test]
async fn success_with_cannot_clone() {
    struct Svc;

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            assert_eq!(req, "hello");
            Ok("world")
        }
    }

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(CannotClone))
        .service(Svc);

    let resp = svc.serve(Context::default(), "hello").await.unwrap();
    assert_eq!(resp, "world");
}

#[tokio::test]
async fn retry_mutating_policy() {
    struct Svc {
        responded: AtomicBool,
        response_counter: Arc<AtomicUsize>,
    }

    impl Service<State, Req> for Svc {
        type Response = Res;
        type Error = Error;

        async fn serve(
            &self,
            _ctx: Context<State>,
            req: Req,
        ) -> Result<Self::Response, Self::Error> {
            self.response_counter.fetch_add(1, Ordering::SeqCst);
            if self.responded.swap(true, Ordering::SeqCst) {
                assert_eq!(req, "retrying");
            } else {
                assert_eq!(req, "hello");
            }
            Ok("world")
        }
    }

    let response_counter = Arc::new(AtomicUsize::new(0));

    let svc = ServiceBuilder::new()
        .layer(RetryLayer::new(MutatingPolicy {
            remaining: Arc::new(Mutex::new(2)),
        }))
        .service(Svc {
            responded: AtomicBool::new(false),
            response_counter: response_counter.clone(),
        });

    let err = svc.serve(Context::default(), "hello").await.unwrap_err();
    assert_eq!(err.to_string(), "out of retries");
    assert_eq!(response_counter.load(Ordering::SeqCst), 3);
}

type State = ();
type Req = &'static str;
type Res = &'static str;
type InnerError = &'static str;
type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct RetryErrors;

impl Policy<State, Req, Res, Error> for RetryErrors {
    async fn retry(
        &self,
        ctx: Context<State>,
        req: Req,
        result: Result<Res, Error>,
    ) -> PolicyResult<State, Req, Res, Error> {
        if result.is_err() {
            PolicyResult::Retry { ctx, req }
        } else {
            PolicyResult::Abort(result)
        }
    }

    fn clone_input(&self, ctx: &Context<State>, req: &Req) -> Option<(Context<State>, Req)> {
        Some((ctx.clone(), *req))
    }
}

#[derive(Clone)]
struct Limit(Arc<Mutex<usize>>);

impl Policy<State, Req, Res, Error> for Limit {
    async fn retry(
        &self,
        ctx: Context<State>,
        req: Req,
        result: Result<Res, Error>,
    ) -> PolicyResult<State, Req, Res, Error> {
        let mut attempts = self.0.lock().unwrap();
        if result.is_err() && *attempts > 0 {
            *attempts -= 1;
            PolicyResult::Retry { ctx, req }
        } else {
            PolicyResult::Abort(result)
        }
    }

    fn clone_input(&self, ctx: &Context<State>, req: &Req) -> Option<(Context<State>, Req)> {
        Some((ctx.clone(), *req))
    }
}

#[derive(Clone)]
struct UnlessErr(InnerError);

impl Policy<State, Req, Res, Error> for UnlessErr {
    async fn retry(
        &self,
        ctx: Context<State>,
        req: Req,
        result: Result<Res, Error>,
    ) -> PolicyResult<State, Req, Res, Error> {
        if result
            .as_ref()
            .err()
            .map(|err| err.to_string() != self.0)
            .unwrap_or_default()
        {
            PolicyResult::Retry { ctx, req }
        } else {
            PolicyResult::Abort(result)
        }
    }

    fn clone_input(&self, ctx: &Context<State>, req: &Req) -> Option<(Context<State>, Req)> {
        Some((ctx.clone(), *req))
    }
}

#[derive(Clone)]
struct CannotClone;

impl Policy<State, Req, Res, Error> for CannotClone {
    async fn retry(
        &self,
        _: Context<State>,
        _: Req,
        _: Result<Res, Error>,
    ) -> PolicyResult<State, Req, Res, Error> {
        unreachable!("retry cannot be called since request isn't cloned");
    }

    fn clone_input(&self, _ctx: &Context<State>, _req: &Req) -> Option<(Context<State>, Req)> {
        None
    }
}

/// Test policy that changes the request to `retrying` during retries and the result to `"out of retries"`
/// when retries are exhausted.
#[derive(Clone)]
struct MutatingPolicy {
    remaining: Arc<Mutex<usize>>,
}

impl Policy<State, Req, Res, Error> for MutatingPolicy
where
    Error: From<&'static str>,
{
    async fn retry(
        &self,
        ctx: Context<State>,
        _req: Req,
        _result: Result<Res, Error>,
    ) -> PolicyResult<State, Req, Res, Error> {
        let mut remaining = self.remaining.lock().unwrap();
        if *remaining == 0 {
            PolicyResult::Abort(Err("out of retries".into()))
        } else {
            *remaining -= 1;
            PolicyResult::Retry {
                ctx,
                req: "retrying",
            }
        }
    }

    fn clone_input(&self, ctx: &Context<State>, req: &Req) -> Option<(Context<State>, Req)> {
        Some((ctx.clone(), *req))
    }
}