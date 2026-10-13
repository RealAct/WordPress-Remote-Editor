import { useLayoutEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type Bounds, type EditorReport, type PostTypeAccess, type SiteContext } from "../lib/api";
import { statusLabel } from "../lib/format";
import { Icon } from "./Icon";

interface Props {
  context: SiteContext;
  postType: PostTypeAccess;
  postId: number | null;
  report: EditorReport | null;
  onBack: () => void;
}

type Device = "Desktop" | "Tablet" | "Mobile";

function measure(el: HTMLElement): Bounds {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, width: r.width, height: r.height };
}

/**
 * Native toolbar plus a placeholder region. The Rust core places the site's
 * block editor (a separate, isolated webview) exactly over the placeholder.
 */
export function EditorView({ context, postType, postId, report, onBack }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [device, setDevice] = useState<Device>("Desktop");

  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    let cancelled = false;
    setError(null);
    api.openEditor(context.site.id, postType.slug, postId, measure(el)).catch((e) => {
      if (!cancelled) setError(errorMessage(e));
    });
    let frame = 0;
    const sync = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => host.current && api.setEditorBounds(measure(host.current)).catch(() => {}));
    };
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    window.addEventListener("resize", sync);
    return () => {
      cancelled = true;
      ro.disconnect();
      window.removeEventListener("resize", sync);
      cancelAnimationFrame(frame);
    };
  }, [context.site.id, postType.slug, postId]);

  const isEditor = report?.kind === "editor";
  const status = report?.status ?? "";
  const canPublish = report?.canPublish ?? postType.canPublish;
  const published = status === "publish" || status === "future" || status === "private";
  const primaryLabel = published ? "Update" : canPublish ? "Publish…" : "Submit for Review";
  const title = isEditor ? report?.title || "Untitled" : postId ? "Loading…" : `New ${postType.singularName}`;

  const cmd = (c: Parameters<typeof api.editorCommand>[0]) => api.editorCommand(c).catch((e) => setError(errorMessage(e)));
  const switchDevice = (d: Device) => {
    setDevice(d);
    void cmd(d === "Desktop" ? "deviceDesktop" : d === "Tablet" ? "deviceTablet" : "deviceMobile");
  };

  return (
    <section className="content editor" aria-label="Editor">
      <header className="toolbar" data-tauri-drag-region>
        <button className="button borderless" onClick={onBack} aria-label={`Back to ${postType.name}`}>
          <Icon name="back" size={16} /> {postType.name}
        </button>
        <div className="toolbar-title-group" data-tauri-drag-region>
          <span className="toolbar-title" data-tauri-drag-region>{title}</span>
          {isEditor && (
            <span className="toolbar-subtitle" data-tauri-drag-region>
              {report?.saving ? "Saving…" : report?.dirty ? "Edited" : statusLabel(status)}
            </span>
          )}
        </div>
        <div className="toolbar-spacer" data-tauri-drag-region />
        {isEditor && (
          <>
            <div className="button-group">
              <button className="icon-button" onClick={() => cmd("undo")} title="Undo" aria-label="Undo">
                <Icon name="undo" />
              </button>
              <button className="icon-button" onClick={() => cmd("redo")} title="Redo" aria-label="Redo">
                <Icon name="redo" />
              </button>
            </div>
            <div className="segmented icons" role="radiogroup" aria-label="Preview size">
              {(["Desktop", "Tablet", "Mobile"] as Device[]).map((d) => (
                <button
                  key={d}
                  role="radio"
                  aria-checked={device === d}
                  className={device === d ? "active" : ""}
                  onClick={() => switchDevice(d)}
                  title={`Preview ${d}`}
                  aria-label={`Preview ${d}`}
                >
                  <Icon name={d.toLowerCase()} size={16} />
                </button>
              ))}
            </div>
            {report?.link && status === "publish" && (
              <button className="icon-button" onClick={() => openUrl(report.link!)} title="View on site" aria-label="View on site">
                <Icon name="external" />
              </button>
            )}
            <button className="icon-button" onClick={() => cmd("toggleSettings")} title="Settings" aria-label="Toggle settings sidebar">
              <Icon name="settings" />
            </button>
            {!published && (
              <button className="button" onClick={() => cmd("save")} disabled={!!report?.saving}>
                Save Draft
              </button>
            )}
            <button className="button primary" onClick={() => cmd("publish")} disabled={!!report?.saving}>
              {primaryLabel}
            </button>
          </>
        )}
      </header>

      {report?.kind === "login" && (
        <div className="banner info">
          Sign in to {context.site.name} once below to open the editor.
          {!context.site.bridgeAvailable && " Install the CreatePress Bridge plugin on the site to skip this step."}
        </div>
      )}
      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}

      <div ref={host} className="editor-host" aria-hidden>
        <div className="editor-loading">Loading the block editor…</div>
      </div>
    </section>
  );
}
