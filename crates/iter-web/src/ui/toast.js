// Toasts (toast.rs): notes in the top right corner that go on their own.
//   #toasts            the region; data-seconds is how long one stays (iter.yaml).
//   .toast.ok/.error   one note, blue or red; an error one is role="alert".
//   [data-echo]        a toast the server rendered (a form's refusal): only armed here.
// The flash cookie (layers/flash.rs) is `ok:<text>` or `error:<text>`, percent-encoded: it is
// shown once and deleted. A toast pauses while hovered or focused and has a close button.
// window.iterToast(kind, text) raises one; iterToast.next(kind, text) leaves one for the
// next page (the cookie), for a script that reloads.
(function () {
  var region = document.getElementById("toasts");
  if (!region) return;
  var COOKIE = "iter_flash";
  var seconds = parseInt(region.dataset.seconds, 10);
  var ms = (seconds > 0 ? seconds : 5) * 1000;

  function close(toast) {
    clearTimeout(toast._timer);
    toast.remove();
  }

  function arm(toast) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "toast-close";
    button.setAttribute("aria-label", "Dismiss");
    button.textContent = "×";
    button.addEventListener("click", function () { close(toast); });
    toast.appendChild(button);
    var held = 0;
    function start() { toast._timer = setTimeout(function () { close(toast); }, ms); }
    function hold() { if (held++ === 0) clearTimeout(toast._timer); }
    function free() { if (--held === 0) start(); }
    toast.addEventListener("mouseenter", hold);
    toast.addEventListener("mouseleave", free);
    toast.addEventListener("focusin", hold);
    toast.addEventListener("focusout", function (e) { if (!toast.contains(e.relatedTarget)) free(); });
    start();
  }

  function show(kind, text) {
    var toast = document.createElement("div");
    toast.className = "toast " + (kind === "error" ? "error" : "ok");
    if (kind === "error") toast.setAttribute("role", "alert");
    var p = document.createElement("p");
    p.className = "toast-text";
    p.textContent = text;
    toast.appendChild(p);
    region.appendChild(toast);
    arm(toast);
    return toast;
  }

  function leave(kind, text) {
    document.cookie = COOKIE + "=" + encodeURIComponent(kind + ":" + text) +
      "; Path=/; Max-Age=60; SameSite=Strict";
  }

  function take() {
    var pairs = document.cookie ? document.cookie.split("; ") : [];
    for (var i = 0; i < pairs.length; i++) {
      if (pairs[i].indexOf(COOKIE + "=") !== 0) continue;
      document.cookie = COOKIE + "=; Path=/; Max-Age=0; SameSite=Strict";
      var value;
      try {
        value = decodeURIComponent(pairs[i].slice(COOKIE.length + 1));
      } catch (_) {
        return null;
      }
      var at = value.indexOf(":");
      if (at < 0 || !value.slice(at + 1)) return null;
      return { kind: value.slice(0, at), text: value.slice(at + 1) };
    }
    return null;
  }

  window.iterToast = show;
  window.iterToast.next = leave;

  Array.prototype.forEach.call(region.querySelectorAll("[data-echo]"), arm);
  var flash = take();
  // A live region reads a change, not what was there on load: add it once the page is up.
  if (flash) setTimeout(function () { show(flash.kind, flash.text); }, 100);
})();
