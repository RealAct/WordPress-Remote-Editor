import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type EditorReport, type PostTypeAccess, type SiteContext, type SiteSummary } from "./lib/api";
import type { Platform } from "./lib/platform";
import { AddSiteSheet } from "./components/AddSiteSheet";
import { EditorView } from "./components/EditorView";
import { Icon } from "./components/Icon";
import { PostList } from "./components/PostList";
import { Sidebar } from "./components/Sidebar";

type View = { kind: "list" } | { kind: "editor"; postId: number | null; key: number };

export function App({ platform }: { platform: Platform }) {
  const [sites, setSites] = useState<SiteSummary[]>([]);
  const [siteId, setSiteId] = useState<string | null>(null);
  const [context, setContext] = useState<SiteContext | null>(null);
  const [contextError, setContextError] = useState<string | null>(null);
  const [postType, setPostType] = useState<PostTypeAccess | null>(null);
  const [view, setView] = useState<View>({ kind: "list" });
  const [report, setReport] = useState<EditorReport | null>(null);
  const [adding, setAdding] = useState(false);
  const [waitingForBrowser, setWaitingForBrowser] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  // Refs so native event handlers always see current state.
  const state = useRef({ view, report, context, postType });
  state.current = { view, report, context, postType };

  const refreshSites = useCallback(async () => {
    const list = await api.listSites();
    setSites(list);
    return list;
  }, []);

  useEffect(() => {
    void refreshSites().then((list) => list[0] && setSiteId((cur) => cur ?? list[0].id));
  }, [refreshSites]);

  // Load what the signed-in user can do on the selected site.
  useEffect(() => {
    setContext(null);
    setPostType(null);
    setContextError(null);
    if (!siteId) return;
    let cancelled = false;
    api
      .getSiteContext(siteId)
      .then((ctx) => {
        if (cancelled) return;
        setContext(ctx);
        setPostType(ctx.postTypes[0] ?? null);
      })
      .catch((e) => !cancelled && setContextError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [siteId, reloadKey]);

  /** Leaves the editor, asking first if there are unsaved changes. */
  const leaveEditor = useCallback(async (): Promise<boolean> => {
    const { view, report } = state.current;
    if (view.kind !== "editor") return true;
    if (report?.dirty) {
      const ok = await ask("You have unsaved changes. Leave the editor and discard them?", {
        title: "Unsaved Changes",
        kind: "warning",
        okLabel: "Discard Changes",
        cancelLabel: "Keep Editing",
      });
      if (!ok) return false;
    }
    await api.closeEditor().catch(() => {});
    setReport(null);
    setView({ kind: "list" });
    return true;
  }, []);

  const openEditor = useCallback((postId: number | null) => {
    setReport(null);
    setView({ kind: "editor", postId, key: Date.now() });
  }, []);

  const selectSite = useCallback(
    async (id: string) => {
      if (await leaveEditor()) setSiteId(id);
    },
    [leaveEditor],
  );

  const selectType = useCallback(
    async (t: PostTypeAccess) => {
      if (await leaveEditor()) setPostType(t);
    },
    [leaveEditor],
  );

  const startAddSite = useCallback(async () => {
    if (await leaveEditor()) {
      setAuthError(null);
      setWaitingForBrowser(false);
      setAdding(true);
    }
  }, [leaveEditor]);

  const removeSite = useCallback(
    async (site: SiteSummary) => {
      const ok = await ask(
        `Remove ${site.name} from this app? Its application password will be revoked on the site and deleted from your keychain.`,
        { title: "Remove Site", kind: "warning", okLabel: "Remove", cancelLabel: "Cancel" },
      );
      if (!ok || !(await leaveEditor())) return;
      try {
        await api.removeSite(site.id);
      } catch (e) {
        await message(errorMessage(e), { title: "Couldn't remove site", kind: "error" });
      }
      const list = await refreshSites();
      setSiteId(list[0]?.id ?? null);
    },
    [leaveEditor, refreshSites],
  );

  const onSiteAdded = useCallback(
    async (site: SiteSummary) => {
      setAdding(false);
      setWaitingForBrowser(false);
      await refreshSites();
      setSiteId(site.id);
    },
    [refreshSites],
  );

  // Native events: auth results, editor state, menu actions.
  useEffect(() => {
    const unlisten = [
      listen<SiteSummary>("auth://completed", (e) => void onSiteAdded(e.payload)),
      listen<string>("auth://failed", (e) => {
        setWaitingForBrowser(false);
        setAuthError(e.payload);
      }),
      listen<EditorReport>("editor://state", (e) => setReport(e.payload)),
      listen<string>("menu://action", (e) => {
        const { view, context } = state.current;
        const inEditor = view.kind === "editor";
        const findType = (slug: string) => context?.postTypes.find((t) => t.slug === slug && t.canCreate);
        switch (e.payload) {
          case "new-post":
          case "new-page": {
            const t = findType(e.payload === "new-post" ? "post" : "page");
            if (!t) return;
            void leaveEditor().then((ok) => {
              if (!ok) return;
              setPostType(t);
              openEditor(null);
            });
            return;
          }
          case "save":
            if (inEditor) void api.editorCommand("save");
            return;
          case "publish":
            if (inEditor) void api.editorCommand("publish");
            return;
          case "toggle-settings":
            if (inEditor) void api.editorCommand("toggleSettings");
            return;
          case "device-desktop":
          case "device-tablet":
          case "device-mobile":
            if (inEditor) {
              const d = e.payload.split("-")[1];
              void api.editorCommand(d === "desktop" ? "deviceDesktop" : d === "tablet" ? "deviceTablet" : "deviceMobile");
            }
            return;
          case "close-editor":
            void leaveEditor();
            return;
          case "add-site":
            void startAddSite();
            return;
          case "toggle-sidebar":
            document.documentElement.classList.toggle("sidebar-hidden");
            window.dispatchEvent(new Event("resize"));
            return;
        }
      }),
    ];
    return () => unlisten.forEach((p) => void p.then((f) => f()));
  }, [leaveEditor, onSiteAdded, openEditor, startAddSite]);

  return (
    <div className="app">
      <Sidebar
        sites={sites}
        selectedSiteId={siteId}
        context={context}
        selectedType={postType?.slug ?? null}
        onSelectSite={selectSite}
        onSelectType={selectType}
        onAddSite={startAddSite}
        onRemoveSite={removeSite}
      />
      <main className="main">
        {sites.length === 0 ? (
          <Welcome onAdd={startAddSite} platform={platform} />
        ) : contextError ? (
          <div className="centered">
            <div className="banner error" role="alert">{contextError}</div>
            <button className="button" onClick={() => setReloadKey((k) => k + 1)}>
              Try Again
            </button>
          </div>
        ) : !context || !postType ? (
          <div className="centered muted">{context ? "Your role can't create or edit content on this site." : "Loading…"}</div>
        ) : view.kind === "editor" ? (
          <EditorView
            key={view.key}
            context={context}
            postType={postType}
            postId={view.postId}
            report={report}
            onBack={() => void leaveEditor()}
          />
        ) : (
          <PostList context={context} postType={postType} onOpen={openEditor} />
        )}
      </main>
      {adding && (
        <AddSiteSheet
          waitingForBrowser={waitingForBrowser}
          authError={authError}
          onStartedAuthorization={() => {
            setAuthError(null);
            setWaitingForBrowser(true);
          }}
          onAdded={onSiteAdded}
          onCancel={() => setAdding(false)}
        />
      )}
    </div>
  );
}

function Welcome({ onAdd, platform }: { onAdd: () => void; platform: Platform }) {
  return (
    <div className="welcome" data-tauri-drag-region>
      <div className="welcome-icon">
        <Icon name="post" size={44} />
      </div>
      <h1>Welcome to WordPress Remote Editor</h1>
      <p>
        Write and publish on your WordPress sites with the full block editor, including blocks from your plugins.
        Your sign-in is kept in the {platform === "macos" ? "macOS Keychain" : platform === "windows" ? "Windows Credential Manager" : "system keyring"}.
      </p>
      <button className="button primary large" onClick={onAdd}>
        Add Your First Site
      </button>
    </div>
  );
}
