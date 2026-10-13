const STATUS_LABELS: Record<string, string> = {
  publish: "Published",
  draft: "Draft",
  pending: "Pending Review",
  future: "Scheduled",
  private: "Private",
  trash: "Trash",
  "auto-draft": "Draft",
};

export function statusLabel(status: string | null | undefined): string {
  if (!status) return "";
  return STATUS_LABELS[status] ?? status.charAt(0).toUpperCase() + status.slice(1);
}

/** WordPress returns `modified` as local site time without a zone. */
export function relativeTime(iso: string | null, now: Date = new Date()): string {
  if (!iso) return "";
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "";
  const seconds = Math.round((now.getTime() - then.getTime()) / 1000);
  const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  const abs = Math.abs(seconds);
  if (abs < 60) return rtf.format(-seconds, "second");
  if (abs < 3600) return rtf.format(-Math.round(seconds / 60), "minute");
  if (abs < 86400) return rtf.format(-Math.round(seconds / 3600), "hour");
  if (abs < 86400 * 7) return rtf.format(-Math.round(seconds / 86400), "day");
  return then.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

/** Turns WordPress role slugs into readable names. */
export function roleLabel(roles: string[]): string {
  const known: Record<string, string> = {
    administrator: "Administrator",
    editor: "Editor",
    author: "Author",
    contributor: "Contributor",
    subscriber: "Subscriber",
  };
  return roles.map((r) => known[r] ?? r.replace(/[_-]+/g, " ")).join(", ");
}

export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}
