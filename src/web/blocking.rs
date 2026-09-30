//! Handlers read and write SQLite with blocking calls (open, WAL switch,
//! migrate, queries, and up to 5 s of `busy_timeout` when the CLI holds the
//! write lock). Run on a tokio worker, that would stall every other task
//! queued on the same thread. This layer runs each request inside
//! `block_in_place`: the runtime moves the worker's other tasks to another
//! thread first, then the handler's future is driven to completion here. One
//! place, no change to any handler. It needs the multi-threaded runtime
//! `serve` builds.

use tokio::runtime::Handle;
use topcoat::{
    Result,
    context::Cx,
    router::{Body, Next, layer, response::Response},
};

#[layer("/")]
pub async fn off_the_workers(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let handle = Handle::current();
    tokio::task::block_in_place(|| handle.block_on(next.run(cx, body)))
}
