//! What a handler takes from the request and the process: the `{id}` of the path, the app (database and clock).

use chrono::NaiveDateTime;
use iter_core::{app::App, db::Db};
use topcoat::{Result, context::Cx, router::path_param};

// The `{id}` of every route with one: a number, else a 400.
path_param!(id: i64, error = bad_request);

pub fn path_id(cx: &Cx) -> Result<i64> {
    Ok(*path_param::<Id>(cx)?)
}

pub fn app() -> Result<App> {
    Ok(App::open()?)
}

/// A new connection: one per request.
pub fn fresh_db() -> Result<Db> {
    Ok(app()?.db)
}

/// The local wall time, without a zone.
pub fn now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_clock_is_the_local_wall_time() {
        let off = (chrono::Local::now().naive_local() - now())
            .num_seconds()
            .abs();
        assert!(off < 5, "{off}s from the local time");
    }
}
