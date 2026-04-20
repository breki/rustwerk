//! Test-only helpers shared between `jira_client` and
//! `lib` unit tests. The recording [`MockHttp`] fake
//! replaces two previously duplicated per-module fakes
//! so that widening [`HttpClient`] (for PLG-JIRA-UPDATE
//! etc.) only has to touch one place.

#![cfg(test)]

use std::cell::RefCell;

use crate::jira_client::{HttpClient, HttpError, HttpResponse};

/// One recorded call made against [`MockHttp`]. Tests
/// pattern-match on this enum to assert that the right
/// HTTP verb + URL + auth header were used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Call {
    Get {
        url: String,
        auth: String,
    },
    Post {
        url: String,
        auth: String,
        body: String,
    },
}

/// Recording fake [`HttpClient`] driven by a pre-loaded
/// queue of canned responses. Pops one response per
/// call in FIFO order; exhausting the queue is a bug in
/// the test, so we panic with a clear message.
pub(crate) struct MockHttp {
    calls: RefCell<Vec<Call>>,
    queue: RefCell<Vec<Result<HttpResponse, HttpError>>>,
}

impl MockHttp {
    pub fn new(queue: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            queue: RefCell::new(queue),
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    fn pop(&self) -> Result<HttpResponse, HttpError> {
        self.queue
            .borrow_mut()
            .drain(..1)
            .next()
            .expect("MockHttp queue drained")
    }
}

impl HttpClient for MockHttp {
    fn get(&self, url: &str, auth: &str) -> Result<HttpResponse, HttpError> {
        self.calls.borrow_mut().push(Call::Get {
            url: url.into(),
            auth: auth.into(),
        });
        self.pop()
    }

    fn post_json(
        &self,
        url: &str,
        auth: &str,
        body: &str,
    ) -> Result<HttpResponse, HttpError> {
        self.calls.borrow_mut().push(Call::Post {
            url: url.into(),
            auth: auth.into(),
            body: body.into(),
        });
        self.pop()
    }
}

/// Build an `Ok` [`HttpResponse`] with the given status
/// and body.
pub(crate) fn ok(status: u16, body: &str) -> Result<HttpResponse, HttpError> {
    Ok(HttpResponse {
        status,
        body: body.into(),
    })
}

/// Build a transport-error `Err` with the given
/// message.
pub(crate) fn transport_err(msg: &str) -> Result<HttpResponse, HttpError> {
    Err(HttpError::Transport(msg.into()))
}
