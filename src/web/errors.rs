//! Pages for the two refusals a person can reach by typing a URL: an id
//! that isn't a number (400) or names nothing (404). Topcoat's own answer is
//! a bare text body; this one has a title, a skip link, landmarks and a way
//! home, in the same shell as every page. Only GET requests are rewritten:
//! the drop script's `fetch` wants the status, not a document.

use topcoat::{
    context::Cx,
    router::{
        Body, LayerFuture, Next, StatusCode,
        content::Html,
        error::{BadRequestError, NotFoundError},
        request::method,
        response::IntoResponse,
    },
};

/// The document for a refusal: static text only, so nothing needs escaping.
fn document(title: &str, message: &str) -> String {
    format!(
        r##"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title} - iter</title><link rel="icon" href="/favicon.ico" sizes="32x32"><link rel="stylesheet" href="/style.css"></head><body><div class="page"><a class="skip" href="#main">Skip to content</a><header class="bar"><a class="brand" href="/">iter</a></header><main id="main" tabindex="-1"><h1>{title}</h1><p>{message}</p><p><a class="u" href="/">Back to the home page</a></p></main></div></body></html>"##
    )
}

/// Registered as a pathless layer (see `web::router`), not with a path: a
/// layer with a path never runs for a request that matched no route, and
/// the unmatched `/nope` is exactly the 404 that needs this page.
pub fn error_pages<'a>(cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
    Box::pin(async move {
        let result = next.run(cx, body).await;
        let Err(error) = result else { return result };
        if !matches!(method(cx).as_str(), "GET" | "HEAD") {
            return Err(error);
        }
        let error = match error.downcast_cloned::<NotFoundError>() {
            Ok(_) => {
                let page = document("Not found", "Nothing lives at this address.");
                return (StatusCode::NOT_FOUND, Html(page)).into_response(cx);
            }
            Err(error) => error,
        };
        match error.downcast_cloned::<BadRequestError>() {
            Ok(_) => {
                let page = document("Bad request", "This address is not valid.");
                (StatusCode::BAD_REQUEST, Html(page)).into_response(cx)
            }
            Err(error) => Err(error),
        }
    })
}

/// What a failed action tells the browser. A refusal the user can act on (a
/// project that is not a git repository, a name already taken, a harness
/// that is not there) is a 400 carrying the message. A fault of the machine
/// (the database, the disk, a program that would not start) is the server's:
/// a 500 with the detail on the terminal, not echoed into a page.
pub fn refused(error: crate::error::IterError) -> topcoat::Error {
    use crate::error::IterError::{Db, Io, Spawn};
    use topcoat::router::error::{bad_request, internal_server_error};
    match error {
        Db(_) | Io(_) | Spawn { .. } => {
            eprintln!("iter: {error}");
            internal_server_error(error).into()
        }
        user => bad_request(user.to_string()).into(),
    }
}
