export function humanSize(bytes?: number | null): string {
  if (bytes == null) return '';
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** Allow only `https:`/`http:` (and `data:` for images). Returns the URL or null. */
export function safeUrl(url: string | null | undefined, allowData = false): string | null {
  if (!url) return null;
  const u = url.trim();
  const lower = u.toLowerCase();
  if (lower.startsWith('https://') || lower.startsWith('http://')) return u;
  if (allowData && lower.startsWith('data:')) return u;
  return null;
}
