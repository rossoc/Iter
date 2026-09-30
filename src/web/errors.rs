//! Pages for the two refusals a person can reach by typing a URL: an id
//! that isn't a number (400) or names nothing (404). Topcoat's own answer is
//! a bare text body; this one has a title, a skip link, landmarks and a way
//! home, in the same shell as every page. Only GET requests are rewritten:
//! the drop script's `fetch` wants the status, not a document.

use topcoat::{
    Result,
    context::Cx,
    router::{
        Body, Next, StatusCode,
        content::Html,
        error::{BadRequestError, NotFoundError},
        layer,
        request::method,
        response::{IntoResponse, Response},
    },
};

/// The document for a refusal: static text only, so nothing needs escaping.
fn document(title: &str, message: &str) -> String {
    format!(
        r##"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title} - iter</title><link rel="stylesheet" href="/v2.css"></head><body><div class="v2"><a class="skip" href="#main">Skip to content</a><header class="bar"><a class="brand" href="/">iter</a></header><main id="main" tabindex="-1"><h1>{title}</h1><p>{message}</p><p><a class="u" href="/">Back to the home page</a></p></main></div></body></html>"##
    )
}

#[layer("/")]
pub async fn error_pages(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
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
}
