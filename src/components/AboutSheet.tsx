import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";

export const DEVELOPER = "Eli Linares";
export const DEVELOPER_URL = "https://realact.net";

export function AboutSheet({ onClose }: { onClose: () => void }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion().then(setVersion, () => setVersion(null));
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div
        className="sheet about"
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        onClick={(e) => e.stopPropagation()}
      >
        <img className="about-icon" src="/app-icon.png" alt="" width={96} height={96} />
        <h2 id="about-title">CreatePress</h2>
        {version && <p className="about-version">Version {version}</p>}
        <p className="about-tagline">Write and publish on your WordPress sites with the full block editor.</p>
        <dl className="about-credits">
          <dt>Developer</dt>
          <dd>{DEVELOPER}</dd>
          <dt>Website</dt>
          <dd>
            <a
              href={DEVELOPER_URL}
              onClick={(e) => {
                e.preventDefault();
                void openUrl(DEVELOPER_URL);
              }}
            >
              realact.net
            </a>
          </dd>
        </dl>
        <p className="about-legal">
          Licensed under the GPL, version 2 or later. WordPress is a trademark of the WordPress Foundation; CreatePress is not affiliated with it.
        </p>
        <div className="sheet-actions">
          <button className="button primary" onClick={onClose} autoFocus>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
