//! The names nearly every file of the crate uses: topcoat's page, route and view items, the
//! request helpers of `cx`, the URL builders and every component of `ui`.

pub use crate::{cx::*, ui::all::*, url::*};
pub use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        page, path_param, query_params,
        response::Response,
        route,
    },
    view::{Child, View, component, view},
};
