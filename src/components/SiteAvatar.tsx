import { useState } from "react";
import type { SiteSummary } from "../lib/api";

/** Site initial in a tinted tile; deterministic colour per site. */
export function SiteAvatar({ site, size = 22 }: { site: Pick<SiteSummary, "name" | "id">; size?: number }) {
  let hash = 0;
  for (const ch of site.id) hash = (hash * 31 + ch.charCodeAt(0)) | 0;
  const hue = Math.abs(hash) % 360;
  return (
    <span
      className="site-avatar"
      style={{ width: size, height: size, background: `hsl(${hue} 55% 48%)`, fontSize: size * 0.5 }}
      aria-hidden
    >
      {(site.name.trim()[0] ?? "W").toUpperCase()}
    </span>
  );
}

export function UserAvatar({ url, name, size = 24 }: { url: string | null; name: string; size?: number }) {
  const [failed, setFailed] = useState(false);
  if (!url || failed) {
    return (
      <span className="user-avatar fallback" style={{ width: size, height: size, fontSize: size * 0.45 }} aria-hidden>
        {(name.trim()[0] ?? "?").toUpperCase()}
      </span>
    );
  }
  return (
    <img
      className="user-avatar"
      src={url}
      width={size}
      height={size}
      alt=""
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
}
