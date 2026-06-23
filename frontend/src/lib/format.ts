/** Small display helpers for technical data (hashes, addresses, amounts). */

/** Middle-truncate a long hash/address: `aedc1cc4…f605ea9`. */
export function truncateMiddle(s: string, head = 8, tail = 7): string {
  if (s.length <= head + tail + 1) return s;
  return `${s.slice(0, head)}…${s.slice(-tail)}`;
}

/** XLM amount from stroops (1 XLM = 1e7 stroops). */
export function stroopsToXlm(stroops: number): string {
  return (stroops / 1e7).toFixed(7).replace(/\.?0+$/, "");
}

/**
 * Elapsed seconds → a compact `mm:ss` clock (`0:07`, `2:14`, `12:30`).
 * Honest: just the wall-clock time the request has spent proving — never a
 * fabricated progress percentage. Clamps negatives to 0.
 */
export function formatElapsed(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const mm = Math.floor(s / 60);
  const ss = s % 60;
  return `${mm}:${ss.toString().padStart(2, "0")}`;
}

/** Copy text to the clipboard; returns true on success. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
