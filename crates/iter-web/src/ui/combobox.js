// One list of choices for every form (combobox.css). An enhancement: native controls
// work without this script.
//   - `input[list]`: a click or ArrowDown shows every choice, typing filters, the first
//     match is highlighted and Enter picks it. Anything may be typed: the server decides.
//   - `select.control`: replaced by a button opening the same list; the hidden <select>
//     stays in sync, so the form posts it, its label points at it and `change` fires on it.
// Both: ArrowUp/Down move, Enter picks, Esc closes (only that: the pop-up under it
// stays), Tab closes, a click picks. Picking sends bubbling `input` and `change`.
(function () {
  var uid = 0;
  var open = null;

  function fire(el, name) {
    el.dispatchEvent(new Event(name, { bubbles: true }));
  }

  // <ul role=listbox> on the body (no `overflow` clips it, and the inert page behind a
  // pop-up does not make it inert), under the anchor or above it when there is no room.
  function Popup(anchor, owner) {
    this.anchor = anchor;
    this.owner = owner;
    this.id = "cb-" + ++uid;
    this.el = document.createElement("ul");
    this.el.className = "cb-pop";
    this.el.id = this.id;
    this.el.setAttribute("role", "listbox");
    this.el.hidden = true;
    document.body.appendChild(this.el);
    this.items = [];
    this.active = -1;
    var self = this;
    this.el.addEventListener("mousedown", function (e) {
      e.preventDefault(); // keep the focus where it is
    });
    this.el.addEventListener("click", function (e) {
      var li = e.target.closest("li[role=option]");
      if (li) owner.pick(self.items[+li.dataset.at]);
    });
    this.el.addEventListener("mousemove", function (e) {
      var li = e.target.closest("li[role=option]");
      if (li && +li.dataset.at !== self.active) self.highlight(+li.dataset.at, false);
    });
  }

  Popup.prototype.show = function (items, active) {
    var self = this;
    this.items = items;
    this.el.textContent = "";
    items.forEach(function (it, at) {
      var li = document.createElement("li");
      li.id = self.id + "-" + at;
      li.setAttribute("role", "option");
      li.dataset.at = at;
      var main = document.createElement("span");
      main.textContent = it.text;
      li.appendChild(main);
      if (it.hint) {
        var hint = document.createElement("span");
        hint.className = "cb-hint";
        hint.textContent = it.hint;
        li.appendChild(hint);
      }
      if (it.selected) li.setAttribute("aria-selected", "true");
      self.el.appendChild(li);
    });
    if (items.length === 0) {
      this.el.hidden = true;
      this.active = -1;
      this.owner.expanded(false, "");
      return;
    }
    this.el.hidden = false;
    if (open && open !== this) open.close();
    open = this;
    this.reposition();
    this.highlight(active, true);
    this.owner.expanded(true, this.id);
  };

  Popup.prototype.highlight = function (at, scroll) {
    var old = this.el.children[this.active];
    if (old) old.classList.remove("cb-active");
    this.active = Math.max(0, Math.min(at, this.items.length - 1));
    var now = this.el.children[this.active];
    if (!now) return;
    now.classList.add("cb-active");
    this.owner.activedescendant(now.id);
    if (scroll) now.scrollIntoView({ block: "nearest" });
  };

  Popup.prototype.move = function (by) {
    this.highlight((this.active + by + this.items.length) % this.items.length, true);
  };

  Popup.prototype.reposition = function () {
    if (this.el.hidden) return;
    var r = this.anchor.getBoundingClientRect();
    var s = this.el.style;
    s.left = r.left + "px";
    s.minWidth = r.width + "px";
    s.maxWidth = Math.max(r.width, Math.min(480, window.innerWidth - r.left - 8)) + "px";
    var h = this.el.offsetHeight;
    var below = window.innerHeight - r.bottom - 8;
    var above = r.top - 8;
    if (h > below && above > below) {
      s.top = "auto";
      s.bottom = window.innerHeight - r.top + 4 + "px";
      s.maxHeight = Math.min(256, above) + "px";
    } else {
      s.bottom = "auto";
      s.top = r.bottom + 4 + "px";
      s.maxHeight = Math.min(256, below) + "px";
    }
  };

  Popup.prototype.close = function () {
    this.el.hidden = true;
    this.active = -1;
    if (open === this) open = null;
    this.owner.expanded(false, "");
  };

  Popup.prototype.isOpen = function () {
    return !this.el.hidden;
  };

  window.addEventListener("resize", function () {
    if (open) open.reposition();
  });
  window.addEventListener("scroll", function () {
    if (open) open.reposition();
  }, true);
  document.addEventListener("mousedown", function (e) {
    if (open && !open.el.contains(e.target) && !open.anchor.contains(e.target)) open.close();
  });

  function enhanceInput(input) {
    var list = document.getElementById(input.getAttribute("list"));
    if (!list) return;
    var choices = function () {
      return Array.prototype.map.call(list.querySelectorAll("option"), function (o) {
        var label = o.label && o.label !== o.value ? o.label : o.textContent.trim();
        return { value: o.value, text: o.value, hint: label && label !== o.value ? label : "" };
      });
    };
    input.removeAttribute("list"); // the native suggestions would show twice
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", "false");
    input.autocomplete = "off";

    var pop = new Popup(input, {
      pick: function (it) {
        input.value = it.value;
        pop.close();
        fire(input, "input");
        fire(input, "change");
      },
      expanded: function (on, id) {
        input.setAttribute("aria-expanded", on ? "true" : "false");
        input.setAttribute("aria-controls", on ? id : "");
        if (!on) input.removeAttribute("aria-activedescendant");
      },
      activedescendant: function (id) {
        input.setAttribute("aria-activedescendant", id);
      },
    });

    function render(all) {
      var q = input.value.trim().toLowerCase();
      var shown = choices().filter(function (o) {
        return all || !q || (o.value + " " + o.hint).toLowerCase().indexOf(q) >= 0;
      });
      var current = -1;
      shown.forEach(function (o, i) {
        o.selected = o.value === input.value;
        if (o.selected && current < 0) current = i;
      });
      pop.show(shown, all && current >= 0 ? current : 0);
    }

    input.addEventListener("focus", function () {
      setTimeout(function () {
        if (document.activeElement === input) input.select();
      }, 0);
    });
    input.addEventListener("mousedown", function () {
      if (!pop.isOpen()) render(true);
    });
    input.addEventListener("mouseup", function (e) {
      if (input.selectionStart === 0 && input.selectionEnd === input.value.length) e.preventDefault();
    });
    input.addEventListener("input", function () {
      render(false);
    });
    input.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        if (pop.isOpen()) pop.move(e.key === "ArrowDown" ? 1 : -1);
        else render(true);
      } else if (e.key === "Enter" && pop.isOpen() && pop.active >= 0) {
        e.preventDefault();
        pop.owner.pick(pop.items[pop.active]);
      } else if (e.key === "Escape" && pop.isOpen()) {
        e.preventDefault();
        e.stopPropagation();
        pop.close();
      } else if (e.key === "Tab") {
        pop.close();
      }
    });
    input.addEventListener("blur", function () {
      pop.close();
    });
  }

  function enhanceSelect(select) {
    var native = select;
    var button = document.createElement("button");
    button.type = "button";
    button.className = select.className + " cb-button";
    button.setAttribute("role", "combobox");
    button.setAttribute("aria-haspopup", "listbox");
    button.setAttribute("aria-expanded", "false");
    var label = select.id ? document.querySelector('label[for="' + select.id + '"]') : null;
    if (label) {
      if (!label.id) label.id = "cb-label-" + ++uid;
      button.setAttribute("aria-labelledby", label.id);
    }
    if (native.getAttribute("aria-describedby")) {
      button.setAttribute("aria-describedby", native.getAttribute("aria-describedby"));
    }
    if (native.getAttribute("aria-invalid")) button.setAttribute("aria-invalid", "true");
    if (native.disabled) button.disabled = true;
    var text = document.createElement("span");
    text.className = "cb-value";
    button.appendChild(text);

    native.classList.add("cb-native");
    native.tabIndex = -1;
    native.setAttribute("aria-hidden", "true");
    native.parentNode.insertBefore(button, native);
    native.addEventListener("focus", function () {
      button.focus();
    });

    function selected() {
      return native.options[native.selectedIndex];
    }
    function sync() {
      var o = selected();
      text.textContent = o ? o.textContent : "";
    }
    sync();
    native.addEventListener("change", sync);

    var pop = new Popup(button, {
      pick: function (it) {
        native.value = it.value;
        sync();
        pop.close();
        fire(native, "input");
        fire(native, "change");
      },
      expanded: function (on, id) {
        button.setAttribute("aria-expanded", on ? "true" : "false");
        if (on) button.setAttribute("aria-controls", id);
        else button.removeAttribute("aria-activedescendant");
      },
      activedescendant: function (id) {
        button.setAttribute("aria-activedescendant", id);
      },
    });

    function show() {
      var items = [];
      var at = 0;
      Array.prototype.forEach.call(native.options, function (o, i) {
        if (o.disabled) return;
        if (i === native.selectedIndex) at = items.length;
        items.push({ value: o.value, text: o.textContent, selected: i === native.selectedIndex });
      });
      pop.show(items, at);
    }

    var typed = "";
    var timer = 0;
    button.addEventListener("click", function () {
      if (pop.isOpen()) pop.close();
      else show();
    });
    button.addEventListener("keydown", function (e) {
      var isOpen = pop.isOpen();
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        if (isOpen) pop.move(e.key === "ArrowDown" ? 1 : -1);
        else show();
      } else if (e.key === "Home" || e.key === "End") {
        if (isOpen) {
          e.preventDefault();
          pop.highlight(e.key === "Home" ? 0 : pop.items.length - 1, true);
        }
      } else if ((e.key === "Enter" || e.key === " ") && isOpen && pop.active >= 0) {
        e.preventDefault();
        pop.owner.pick(pop.items[pop.active]);
      } else if (e.key === "Enter" && !isOpen) {
        e.preventDefault();
        if (button.form) button.form.requestSubmit();
      } else if (e.key === "Escape" && isOpen) {
        e.preventDefault();
        e.stopPropagation();
        pop.close();
      } else if (e.key === "Tab") {
        pop.close();
      } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && e.key !== " ") {
        // type-ahead: the first choice starting with what was typed
        typed += e.key.toLowerCase();
        clearTimeout(timer);
        timer = setTimeout(function () {
          typed = "";
        }, 700);
        if (!isOpen) show();
        for (var i = 0; i < pop.items.length; i++) {
          if (pop.items[i].text.toLowerCase().indexOf(typed) === 0) {
            pop.highlight(i, true);
            break;
          }
        }
      }
    });
    button.addEventListener("blur", function () {
      pop.close();
    });
  }

  function enhance(root) {
    Array.prototype.forEach.call(root.querySelectorAll("input[list]"), function (el) {
      if (!el.dataset.cb) {
        el.dataset.cb = "1";
        enhanceInput(el);
      }
    });
    Array.prototype.forEach.call(root.querySelectorAll("select.control"), function (el) {
      if (!el.dataset.cb && !el.multiple && !(el.size > 1)) {
        el.dataset.cb = "1";
        enhanceSelect(el);
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      enhance(document);
    });
  } else {
    enhance(document);
  }
})();
