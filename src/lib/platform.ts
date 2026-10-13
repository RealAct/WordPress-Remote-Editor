export type Platform = "macos" | "windows" | "linux";

/** Resolves the host OS so the shell can adopt that platform's conventions. */
export async function detectPlatform(): Promise<Platform> {
  try {
    const os = await import("@tauri-apps/plugin-os");
    const p = os.platform();
    if (p === "macos" || p === "windows") return p;
    return "linux";
  } catch {
    const ua = navigator.userAgent;
    if (/Mac/.test(ua)) return "macos";
    if (/Windows/.test(ua)) return "windows";
    return "linux";
  }
}

export function modKey(platform: Platform): string {
  return platform === "macos" ? "⌘" : "Ctrl+";
}
