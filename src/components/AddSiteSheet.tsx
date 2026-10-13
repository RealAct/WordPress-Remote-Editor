import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type SiteDiscovery, type SiteSummary } from "../lib/api";
import { hostOf } from "../lib/format";
import { Icon } from "./Icon";

interface Props {
  waitingForBrowser: boolean;
  authError: string | null;
  onStartedAuthorization: () => void;
  onAdded: (site: SiteSummary) => void;
  onCancel: () => void;
}

type Step = "address" | "signin" | "manual";

export function AddSiteSheet({ waitingForBrowser, authError, onStartedAuthorization, onAdded, onCancel }: Props) {
  const [step, setStep] = useState<Step>("address");
  const [url, setUrl] = useState("");
  const [allowLocal, setAllowLocal] = useState(false);
  const [discovery, setDiscovery] = useState<SiteDiscovery | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const firstField = useRef<HTMLInputElement>(null);

  useEffect(() => firstField.current?.focus(), [step]);
  useEffect(() => setError(authError), [authError]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onCancel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  async function findSite(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const d = await api.discoverSite(url, allowLocal);
      setDiscovery(d);
      setStep(d.supportsApplicationPasswords ? "signin" : "manual");
      if (!d.supportsApplicationPasswords) {
        setError("This site doesn't advertise Application Password sign-in. You can still try an application password created under Users → Profile.");
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function signInWithWordPress() {
    setBusy(true);
    setError(null);
    try {
      await api.beginAuthorization(discovery?.siteUrl ?? url, allowLocal);
      onStartedAuthorization();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function addManually(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const site = await api.addSiteManual(discovery?.siteUrl ?? url, username, password, allowLocal);
      setPassword("");
      onAdded(site);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="sheet-backdrop" role="presentation" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="add-site-title">
        <h2 id="add-site-title">Add a WordPress Site</h2>

        {step === "address" && (
          <form onSubmit={findSite}>
            <p className="sheet-intro">Enter the address of a site you can sign in to.</p>
            <label className="field">
              <span>Site address</span>
              <input
                ref={firstField}
                type="text"
                inputMode="url"
                placeholder="example.com"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                required
              />
            </label>
            <label className="checkbox">
              <input type="checkbox" checked={allowLocal} onChange={(e) => setAllowLocal(e.target.checked)} />
              <span>This is a local development site (allow HTTP on localhost, .local and .test)</span>
            </label>
            {error && <p className="form-error" role="alert">{error}</p>}
            <div className="sheet-actions">
              <button type="button" className="button" onClick={onCancel}>
                Cancel
              </button>
              <button type="submit" className="button primary" disabled={busy || !url.trim()}>
                {busy ? "Looking…" : "Continue"}
              </button>
            </div>
          </form>
        )}

        {step === "signin" && discovery && (
          <div>
            <div className="site-card">
              <Icon name="globe" size={28} />
              <div>
                <div className="site-card-name">{discovery.name}</div>
                <div className="site-card-host">{hostOf(discovery.siteUrl)}</div>
              </div>
            </div>
            {waitingForBrowser ? (
              <p className="sheet-intro waiting">
                Approve <strong>WordPress Remote Editor</strong> in the sign-in window. This sheet updates when you're done.
              </p>
            ) : (
              <p className="sheet-intro">
                You'll sign in on your site and approve this app. WordPress creates an Application Password just for this app, which you can revoke from your profile at any time.
              </p>
            )}
            {error && <p className="form-error" role="alert">{error}</p>}
            <div className="sheet-actions spread">
              <button type="button" className="link-button" onClick={() => setStep("manual")}>
                Use an application password instead
              </button>
              <div className="sheet-actions">
                <button type="button" className="button" onClick={onCancel}>
                  Cancel
                </button>
                <button type="button" className="button primary" onClick={signInWithWordPress} disabled={busy}>
                  <Icon name="lock" size={14} /> {waitingForBrowser ? "Open Sign-in Again" : "Sign In with WordPress"}
                </button>
              </div>
            </div>
          </div>
        )}

        {step === "manual" && (
          <form onSubmit={addManually} autoComplete="off">
            <p className="sheet-intro">
              Create an Application Password in WordPress under <strong>Users → Profile → Application Passwords</strong>, then paste it here. It's stored only in your system keychain.
            </p>
            <label className="field">
              <span>Username</span>
              <input
                ref={firstField}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                required
              />
            </label>
            <label className="field">
              <span>Application password</span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="xxxx xxxx xxxx xxxx xxxx xxxx"
                spellCheck={false}
                required
              />
            </label>
            {error && <p className="form-error" role="alert">{error}</p>}
            <div className="sheet-actions spread">
              <button type="button" className="link-button" onClick={() => setStep(discovery?.supportsApplicationPasswords ? "signin" : "address")}>
                Back
              </button>
              <div className="sheet-actions">
                <button type="button" className="button" onClick={onCancel}>
                  Cancel
                </button>
                <button type="submit" className="button primary" disabled={busy}>
                  {busy ? "Checking…" : "Add Site"}
                </button>
              </div>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
