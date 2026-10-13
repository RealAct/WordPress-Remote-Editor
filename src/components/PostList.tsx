import { useCallback, useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type PostSummary, type PostTypeAccess, type SiteContext, type StatusFilter } from "../lib/api";
import { relativeTime, statusLabel } from "../lib/format";
import { Icon } from "./Icon";

interface Props {
  context: SiteContext;
  postType: PostTypeAccess;
  onOpen: (postId: number | null) => void;
}

const FILTERS: { id: StatusFilter; label: string }[] = [
  { id: "any", label: "All" },
  { id: "draft", label: "Drafts" },
  { id: "pending", label: "Pending" },
  { id: "future", label: "Scheduled" },
  { id: "publish", label: "Published" },
];

export function PostList({ context, postType, onOpen }: Props) {
  const [filter, setFilter] = useState<StatusFilter>("any");
  const [mineOnly, setMineOnly] = useState(!postType.canEditOthers);
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<PostSummary[]>([]);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<number | null>(null);

  useEffect(() => setMineOnly(!postType.canEditOthers), [postType]);

  // Debounce search input.
  useEffect(() => {
    const t = window.setTimeout(() => setQuery(search), 300);
    return () => window.clearTimeout(t);
  }, [search]);

  const load = useCallback(
    async (nextPage: number) => {
      setLoading(true);
      setError(null);
      try {
        const res = await api.listPosts({
          siteId: context.site.id,
          restBase: postType.restBase,
          status: filter,
          search: query,
          page: nextPage,
          mineOnly,
          canEditOthers: postType.canEditOthers,
          userId: context.userId,
        });
        setItems((prev) => (nextPage === 1 ? res.items : [...prev, ...res.items]));
        setPage(res.page);
        setTotalPages(res.totalPages);
      } catch (e) {
        setError(errorMessage(e));
        if (nextPage === 1) setItems([]);
      } finally {
        setLoading(false);
      }
    },
    [context.site.id, context.userId, postType, filter, query, mineOnly],
  );

  useEffect(() => {
    void load(1);
  }, [load]);

  async function trash(p: PostSummary) {
    const ok = await ask(`Move “${p.title || "(no title)"}” to the Trash? You can restore it from WordPress.`, {
      title: "Move to Trash",
      kind: "warning",
      okLabel: "Move to Trash",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await api.trashPost(context.site.id, postType.restBase, p.id);
      setItems((prev) => prev.filter((x) => x.id !== p.id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <section className="content" aria-label={postType.name}>
      <header className="toolbar" data-tauri-drag-region>
        <h1 className="toolbar-title" data-tauri-drag-region>{postType.name}</h1>
        <div className="segmented" role="tablist" aria-label="Filter by status">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              role="tab"
              aria-selected={filter === f.id}
              className={filter === f.id ? "active" : ""}
              onClick={() => setFilter(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>
        <div className="toolbar-spacer" data-tauri-drag-region />
        <label className="search-field">
          <Icon name="search" size={14} />
          <input
            type="search"
            placeholder="Search"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            aria-label={`Search ${postType.name}`}
          />
        </label>
        <button className="icon-button" onClick={() => load(1)} title="Refresh" aria-label="Refresh">
          <Icon name="refresh" />
        </button>
        {postType.canCreate && (
          <button className="button primary" onClick={() => onOpen(null)}>
            <Icon name="plus" size={14} /> New {postType.singularName}
          </button>
        )}
      </header>

      {postType.canEditOthers && (
        <div className="subbar">
          <label className="checkbox small">
            <input type="checkbox" checked={mineOnly} onChange={(e) => setMineOnly(e.target.checked)} />
            <span>Only mine</span>
          </label>
        </div>
      )}

      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}

      <div className="list" role="listbox" aria-label={`${postType.name} list`}>
        {items.map((p) => (
          <div
            key={p.id}
            role="option"
            aria-selected={selected === p.id}
            tabIndex={0}
            className={"row" + (selected === p.id ? " selected" : "") + (p.canEdit ? "" : " readonly")}
            onClick={() => setSelected(p.id)}
            onDoubleClick={() => p.canEdit && onOpen(p.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && p.canEdit) onOpen(p.id);
            }}
          >
            <div className="row-main">
              <div className="row-title">{p.title || <span className="muted">(no title)</span>}</div>
              <div className="row-meta">
                <span className={`status status-${p.status}`}>{statusLabel(p.status)}</span>
                <span className="muted">{relativeTime(p.modified)}</span>
              </div>
            </div>
            <div className="row-actions">
              {p.link && p.status === "publish" && (
                <button className="icon-button" title="View on site" aria-label="View on site" onClick={() => openUrl(p.link!)}>
                  <Icon name="external" />
                </button>
              )}
              {p.canEdit && postType.canDelete && (
                <button className="icon-button danger" title="Move to Trash" aria-label="Move to Trash" onClick={() => trash(p)}>
                  <Icon name="trash" />
                </button>
              )}
              {p.canEdit && (
                <button className="button small" onClick={() => onOpen(p.id)}>
                  Edit
                </button>
              )}
            </div>
          </div>
        ))}

        {!loading && items.length === 0 && !error && (
          <div className="empty">
            <Icon name={postType.slug === "page" ? "page" : "post"} size={40} />
            <p>No {postType.name.toLowerCase()} here yet.</p>
            {postType.canCreate && (
              <button className="button primary" onClick={() => onOpen(null)}>
                New {postType.singularName}
              </button>
            )}
          </div>
        )}

        {loading && <div className="loading" aria-live="polite">Loading…</div>}
        {!loading && page < totalPages && (
          <div className="load-more">
            <button className="button" onClick={() => load(page + 1)}>
              Load More
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
