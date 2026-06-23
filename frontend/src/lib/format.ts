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

/** Copy text to the clipboard; returns true on success. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
