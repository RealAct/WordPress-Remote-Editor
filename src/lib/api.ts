// Typed wrappers around the Rust core's IPC commands. The shell never talks
// to WordPress directly and never handles passwords after submitting them.
import { invoke } from "@tauri-apps/api/core";

export interface SiteSummary {
  id: string;
  name: string;
  url: string;
  username: string;
  displayName: string;
  avatarUrl: string | null;
  bridgeAvailable: boolean;
}

export interface SiteDiscovery {
  siteUrl: string;
  adminUrl: string;
  apiRoot: string;
  name: string;
  description: string;
  supportsApplicationPasswords: boolean;
  bridgeAvailable: boolean;
}

export interface PostTypeAccess {
  slug: string;
  restBase: string;
  name: string;
  singularName: string;
  hierarchical: boolean;
  canCreate: boolean;
  canPublish: boolean;
  canEditOthers: boolean;
  canDelete: boolean;
}

export interface SiteContext {
  site: SiteSummary;
  userId: number;
  roles: string[];
  postTypes: PostTypeAccess[];
}

export interface PostSummary {
  id: number;
  title: string;
  status: string;
  modified: string | null;
  author: number | null;
  link: string | null;
  canEdit: boolean;
}

export interface PostPage {
  items: PostSummary[];
  total: number;
  totalPages: number;
  page: number;
}

export interface EditorReport {
  kind: "editor" | "login" | "page";
  postId?: number | null;
  postType?: string | null;
  title?: string | null;
  status?: string | null;
  link?: string | null;
  dirty?: boolean | null;
  saving?: boolean | null;
  canPublish?: boolean | null;
  path?: string | null;
}

export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type EditorCommand =
  | "save"
  | "publish"
  | "undo"
  | "redo"
  | "toggleSettings"
  | "deviceDesktop"
  | "deviceTablet"
  | "deviceMobile";

export type StatusFilter = "any" | "draft" | "pending" | "future" | "publish" | "private" | "trash";

export const api = {
  discoverSite: (url: string, allowInsecureLocal = false) =>
    invoke<SiteDiscovery>("discover_site", { url, allowInsecureLocal }),
  beginAuthorization: (url: string, allowInsecureLocal = false) =>
    invoke<void>("begin_authorization", { url, allowInsecureLocal }),
  addSiteManual: (url: string, username: string, appPassword: string, allowInsecureLocal = false) =>
    invoke<SiteSummary>("add_site_manual", { url, username, appPassword, allowInsecureLocal }),
  listSites: () => invoke<SiteSummary[]>("list_sites"),
  removeSite: (siteId: string) => invoke<void>("remove_site", { siteId }),
  getSiteContext: (siteId: string) => invoke<SiteContext>("get_site_context", { siteId }),
  listPosts: (args: {
    siteId: string;
    restBase: string;
    status: StatusFilter;
    search: string;
    page: number;
    mineOnly: boolean;
    canEditOthers: boolean;
    userId: number;
  }) => invoke<PostPage>("list_posts", { query: args }),
  trashPost: (siteId: string, restBase: string, postId: number) =>
    invoke<void>("trash_post", { siteId, restBase, postId }),
  openEditor: (siteId: string, postType: string, postId: number | null, bounds: Bounds) =>
    invoke<void>("open_editor", { siteId, postType, postId, bounds }),
  closeEditor: () => invoke<void>("close_editor"),
  setEditorBounds: (bounds: Bounds) => invoke<void>("editor_set_bounds", { bounds }),
  editorCommand: (command: EditorCommand) => invoke<void>("editor_command", { command }),
};

/** Errors from the core arrive as plain user-facing strings. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong.";
}
