//! Runs each request inside `block_in_place`: handlers make blocking SQLite calls (up to 5 s of `busy_timeout`) and need the multi-threaded runtime.

use crate::prelude::*;
use tokio::runtime::Handle;
use topcoat::router::Body;
use topcoat::router::Next;
use topcoat::router::layer;

#[layer("/")]
pub async fn off_the_workers(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let handle = Handle::current();
    tokio::task::block_in_place(|| handle.block_on(next.run(cx, body)))
}
