//! What every component imports: topcoat's page, route and view items, the request helpers of
//! `cx` and the URL builders. `prelude` adds the components themselves on top.

pub use crate::{cx::*, url::*};
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
