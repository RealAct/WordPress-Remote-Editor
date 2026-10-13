import type { PostTypeAccess, SiteContext, SiteSummary } from "../lib/api";
import { roleLabel } from "../lib/format";
import { Icon } from "./Icon";
import { SiteAvatar, UserAvatar } from "./SiteAvatar";

interface Props {
  sites: SiteSummary[];
  selectedSiteId: string | null;
  context: SiteContext | null;
  selectedType: string | null;
  onSelectSite: (id: string) => void;
  onSelectType: (t: PostTypeAccess) => void;
  onAddSite: () => void;
  onRemoveSite: (site: SiteSummary) => void;
  onAbout: () => void;
}

export function Sidebar(props: Props) {
  const { sites, selectedSiteId, context, selectedType } = props;
  return (
    <nav className="sidebar" aria-label="Sites and content">
      <div className="sidebar-titlebar" data-tauri-drag-region />
      <div className="sidebar-scroll">
        <div className="sidebar-section">
          <div className="sidebar-heading">Sites</div>
          <ul role="list">
            {sites.map((s) => (
              <li key={s.id}>
                <button
                  className={"sidebar-item" + (s.id === selectedSiteId ? " selected" : "")}
                  onClick={() => props.onSelectSite(s.id)}
                  aria-current={s.id === selectedSiteId ? "true" : undefined}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    props.onRemoveSite(s);
                  }}
                >
                  <SiteAvatar site={s} />
                  <span className="label">{s.name}</span>
                </button>
              </li>
            ))}
            <li>
              <button className="sidebar-item subtle" onClick={props.onAddSite}>
                <span className="icon-tile">
                  <Icon name="plus" size={14} />
                </span>
                <span className="label">Add Site…</span>
              </button>
            </li>
          </ul>
        </div>

        {context && (
          <div className="sidebar-section">
            <div className="sidebar-heading">Content</div>
            <ul role="list">
              {context.postTypes.map((t) => (
                <li key={t.slug}>
                  <button
                    className={"sidebar-item" + (t.slug === selectedType ? " selected" : "")}
                    onClick={() => props.onSelectType(t)}
                  >
                    <Icon name={t.slug === "page" ? "page" : t.slug === "post" ? "post" : "type"} />
                    <span className="label">{t.name}</span>
                    {!t.canPublish && t.canCreate && (
                      <span className="badge" title="Your role can submit these for review">
                        Review
                      </span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      <div className="sidebar-about">
        <button className="sidebar-item subtle" onClick={props.onAbout}>
          <span className="icon-tile">
            <Icon name="info" size={14} />
          </span>
          <span className="label">About CreatePress</span>
        </button>
      </div>

      {context && (
        <div className="sidebar-footer">
          <UserAvatar url={context.site.avatarUrl} name={context.site.displayName} />
          <div className="who">
            <div className="name">{context.site.displayName}</div>
            <div className="role">{roleLabel(context.roles)}</div>
          </div>
          <button
            className="icon-button"
            title="Remove this site"
            aria-label="Remove this site"
            onClick={() => props.onRemoveSite(context.site)}
          >
            <Icon name="more" />
          </button>
        </div>
      )}
    </nav>
  );
}
