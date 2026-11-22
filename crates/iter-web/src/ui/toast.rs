//! The toasts: short notes in the top right corner, blue for a success and red for an
//! error, gone after the configured seconds (`notifications` in `iter.yaml`). The region is
//! in every frame; `toast.js` fills it from the flash cookie (`web/flash.rs`) and with
//! `window.iterToast(kind, text)`. Turned off, neither the region nor the script is there.

use crate::prelude::*;
use iter_core::config::config;

/// The region, live (`polite`) so a success is read out; an error toast that `toast.js`
/// adds is `role="alert"`. A form's own refusal is rendered here already, without the
/// role: the form's error box is the one announced and focused, this is its visual echo.
#[component]
pub async fn toasts(#[default] error: Option<&FormError>) -> Result<impl View> {
    let settings = &config().notifications;
    let enabled = settings.enabled;
    let seconds = settings.seconds.to_string();
    Ok(view! {
        if enabled {
            <div class="toasts" id="toasts" aria-live="polite" data-seconds=(seconds.as_str())>
                if let Some(error) = error {
                    <div class="toast error" data-echo=""><p class="toast-text">(error.message.as_str())</p></div>
                }
            </div>
            <script src="/toast.js" defer=""></script>
        }
    })
}
