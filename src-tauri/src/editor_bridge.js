// Injected into the editor webview (main frame only). It runs inside the
// site's own wp-admin, so it must treat the page as untrusted: it only reads
// editor state and reports a small, fixed-shape object to the native shell.
(function () {
  "use strict";
  var CONFIG = __WPRE_CONFIG__;
  if (window.top !== window) return;
  if (CONFIG.origins.indexOf(window.location.origin) === -1) return;

  // ---- Native look: hide wp-admin chrome around the block editor ----------
  var css =
    "html.wp-toolbar{padding-top:0!important}" +
    "#wpadminbar,#adminmenumain,#wpfooter,.edit-post-fullscreen-mode-close," +
    ".editor-header__back-button,.edit-post-header__back-button{display:none!important}" +
    "#wpcontent,#wpbody-content{margin-left:0!important;padding-bottom:0!important}" +
    ".interface-interface-skeleton{top:0!important;left:0!important}" +
    "body.is-fullscreen-mode .interface-interface-skeleton{top:0!important}";
  function injectStyle() {
    if (document.getElementById("wpre-embedded-style")) return;
    var style = document.createElement("style");
    style.id = "wpre-embedded-style";
    style.textContent = css;
    (document.head || document.documentElement).appendChild(style);
    document.documentElement.classList.add("wpre-embedded");
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", injectStyle);
  } else {
    injectStyle();
  }

  // ---- Reporting --------------------------------------------------------
  function report(payload) {
    try {
      var ipc = window.__TAURI_INTERNALS__;
      if (ipc && typeof ipc.invoke === "function") {
        ipc.invoke("editor_report", { report: payload });
      }
    } catch (e) {
      /* the shell simply won't update */
    }
  }

  var path = window.location.pathname;
  if (/\/wp-login\.php$/.test(path)) {
    report({ kind: "login" });
    return;
  }

  function editorStore() {
    var wp = window.wp;
    return wp && wp.data && wp.data.select && wp.data.select("core/editor");
  }

  function hasPublishAction(s) {
    var post = s.getCurrentPost && s.getCurrentPost();
    return !!(post && post._links && post._links["wp:action-publish"]);
  }

  function snapshot(s) {
    var title = s.getEditedPostAttribute("title");
    return {
      kind: "editor",
      postId: s.getCurrentPostId() || null,
      postType: String(s.getCurrentPostType() || "").slice(0, 64),
      title: String(title || "").slice(0, 500),
      status: String(s.getEditedPostAttribute("status") || "").slice(0, 32),
      link: String(s.getPermalink ? s.getPermalink() || "" : "").slice(0, 2048),
      dirty: !!s.isEditedPostDirty(),
      saving: !!s.isSavingPost() && !s.isAutosavingPost(),
      canPublish: hasPublishAction(s),
    };
  }

  function start() {
    var s = editorStore();
    if (!s || !s.getCurrentPostId) return false;
    var last = "";
    var pending = false;
    window.wp.data.subscribe(function () {
      if (pending) return;
      pending = true;
      // Coalesce bursts of store updates into one report per frame.
      window.requestAnimationFrame(function () {
        pending = false;
        var snap = snapshot(editorStore());
        var key = JSON.stringify(snap);
        if (key !== last) {
          last = key;
          report(snap);
        }
      });
    });
    report(snapshot(s));
    return true;
  }

  // ---- Commands the native toolbar can trigger --------------------------
  function click(selectors) {
    for (var i = 0; i < selectors.length; i++) {
      var el = document.querySelector(selectors[i]);
      if (el && !el.disabled && el.getAttribute("aria-disabled") !== "true") {
        el.click();
        return true;
      }
    }
    return false;
  }

  Object.defineProperty(window, "__wpre", {
    configurable: false,
    writable: false,
    value: Object.freeze({
      save: function () {
        var d = window.wp.data.dispatch("core/editor");
        d.savePost();
      },
      // Use Gutenberg's own publish button so pre-publish checks and any
      // plugin hooks on it (SEO, scheduling, review) still run.
      publish: function () {
        if (
          !click([
            ".editor-post-publish-panel__toggle",
            ".editor-post-publish-button__button",
            ".editor-post-publish-button",
          ])
        ) {
          var d = window.wp.data.dispatch("core/editor");
          var s = editorStore();
          d.editPost({ status: hasPublishAction(s) ? "publish" : "pending" });
          d.savePost();
        }
      },
      device: function (type) {
        var allowed = { Desktop: 1, Tablet: 1, Mobile: 1 };
        if (!allowed[type]) return;
        var d = window.wp.data.dispatch("core/editor");
        if (d.setDeviceType) d.setDeviceType(type);
      },
      undo: function () {
        window.wp.data.dispatch("core/editor").undo();
      },
      redo: function () {
        window.wp.data.dispatch("core/editor").redo();
      },
      toggleSettings: function () {
        click([
          'button[aria-controls="edit-post:document"]',
          'button[aria-label="Settings"]',
        ]);
      },
    }),
  });

  var tries = 0;
  var timer = window.setInterval(function () {
    tries += 1;
    if (start() || tries > 120) {
      window.clearInterval(timer);
      if (tries > 120) report({ kind: "page", path: path.slice(0, 512) });
    }
  }, 250);
})();
