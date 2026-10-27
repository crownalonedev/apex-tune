/**
 * Update checks against a GitHub "latest release" endpoint, configured at
 * build time with VITE_UPDATE_URL (e.g.
 * https://api.github.com/repos/<owner>/<repo>/releases/latest). Without it,
 * the app says so instead of pretending to check.
 */
export const UPDATE_URL: string | undefined = import.meta.env.VITE_UPDATE_URL || undefined;

export type UpdateResult =
  | { kind: "notConfigured" }
  | { kind: "upToDate"; latest: string }
  | { kind: "available"; latest: string; url: string }
  | { kind: "failed"; message: string };

/** Compares dotted versions, ignoring a leading "v". */
export function isNewer(candidate: string, current: string): boolean {
  const parse = (v: string) => v.replace(/^v/i, "").split(/[.-]/).map((p) => Number.parseInt(p, 10) || 0);
  const a = parse(candidate);
  const b = parse(current);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  return false;
}

export async function checkForUpdate(current: string, url = UPDATE_URL): Promise<UpdateResult> {
  if (!url) return { kind: "notConfigured" };
  try {
    const res = await fetch(url, { headers: { Accept: "application/vnd.github+json" } });
    if (!res.ok) return { kind: "failed", message: `The update server answered ${res.status}.` };
    const body = (await res.json()) as { tag_name?: string; html_url?: string };
    const latest = body.tag_name ?? "";
    if (!latest) return { kind: "failed", message: "The update server's reply had no version." };
    return isNewer(latest, current)
      ? { kind: "available", latest, url: body.html_url ?? "" }
      : { kind: "upToDate", latest };
  } catch {
    return { kind: "failed", message: "Couldn't reach the update server. Check your connection." };
  }
}
