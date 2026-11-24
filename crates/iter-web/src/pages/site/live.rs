//! `GET /live`: the live version (`iter_core::live`) as server-sent events, so a page knows when
//! what it shows may have changed (`ui/live.js` then swaps its `[data-live]` regions). The
//! first event is the version at once; then one each time it changes, checked every second. The
//! version is `<pages>.<logs>` (`Parts`): a page that does not follow a run's log ignores the second.

use crate::prelude::*;
use futures_core::Stream;
use iter_core::live::{Parts, Version};
use std::time::Duration;
use topcoat::router::content::sse::{Event, KeepAlive, Sse};

const EVERY: Duration = Duration::from_secs(1);

struct Watch {
    version: Version,
    last: Option<Parts>,
}

/// The version now, asked off the async workers: it reads the database and walks folders.
async fn check(version: Version) -> Option<(Version, Parts)> {
    tokio::task::spawn_blocking(move || {
        let now = version.parts();
        (version, now)
    })
    .await
    .ok()
}

#[route(GET "/live")]
pub(super) async fn live() -> Result<Sse<impl Stream<Item = Result<Event>> + use<>>> {
    // the handler only opens the connection; the stream runs after it returned, on the
    // workers, so it holds no blocking thread between checks
    let version = Version::open(&app()?)?;
    let watch = Watch {
        version,
        last: None,
    };
    let events = futures_util::stream::unfold(watch, |mut watch| async move {
        loop {
            if watch.last.is_some() {
                tokio::time::sleep(EVERY).await;
            }
            let (version, now) = check(watch.version).await?;
            watch.version = version;
            if watch.last != Some(now) {
                watch.last = Some(now);
                let event = Event::new().data(now.to_string());
                return Some((Ok(event), watch));
            }
        }
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::new()))
}
