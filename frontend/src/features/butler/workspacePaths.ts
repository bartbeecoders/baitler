/** Detecting typed workspace paths in a butler message.
 *
 * A spawned run can only read a folder it was *granted* (`workspace_dir` →
 * `--add-dir`); the agent cannot acquire access mid-run. So when the user types
 * an allow-listed path into the composer instead of using the picker ("I mean
 * /run/media/bart/Development/Projects"), treat that as intent to attach it —
 * otherwise the run is doomed to a permission error the user can't approve.
 */

/** First absolute path in `text` that falls under one of the allow-listed
 * `roots` (or is a root itself), normalized without a trailing slash. The
 * server re-validates (canonicalize-under-roots), so this only needs to decide
 * whether attaching is plausibly what the user meant. */
export function detectRootedPath(text: string, roots: string[]): string | null {
  const normalRoots = roots.map((r) => r.replace(/\/+$/, '')).filter(Boolean);
  if (normalRoots.length === 0) return null;
  // Candidate tokens: absolute paths, stopping at whitespace or quotes; strip
  // likely sentence punctuation and any trailing slash.
  const candidates = text.match(/\/[^\s"'`]+/g) ?? [];
  for (const raw of candidates) {
    const path = raw.replace(/[.,;:!?)\]}>]+$/, '').replace(/\/+$/, '');
    if (normalRoots.some((r) => path === r || path.startsWith(`${r}/`))) return path;
  }
  return null;
}
