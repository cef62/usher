const LATEST = 'https://github.com/cef62/usher/releases/latest';

// Latest published release dmg. Any failure falls back to the /releases/latest page so the
// build never breaks or links to a missing asset.
export async function getDownload(): Promise<{ version?: string; dmg: string }> {
  try {
    const token = process.env.GITHUB_TOKEN;
    const res = await fetch('https://api.github.com/repos/cef62/usher/releases/latest', {
      headers: { Accept: 'application/vnd.github+json', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
      signal: AbortSignal.timeout(10_000),
    });
    if (!res.ok) return { dmg: LATEST };
    const rel = await res.json();
    const urls: string[] = (rel.assets ?? []).map((a: { browser_download_url: string }) => a.browser_download_url);
    return { version: String(rel.tag_name).replace(/^v/, ''), dmg: urls.find((u) => u.endsWith('_aarch64.dmg')) ?? LATEST };
  } catch {
    return { dmg: LATEST };
  }
}
