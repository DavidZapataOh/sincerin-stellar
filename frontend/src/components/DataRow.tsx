/*
 * Data row — label (text) + value (mono), hairline-separated. Optional copy +
 * explorer affordances for hashes / addresses. Mono is reserved for genuine data.
 */

import { useEffect, useRef, useState } from "react";
import { copyText, truncateMiddle } from "../lib/format";

interface Props {
  label: string;
  value: string;
  /** Show a truncated form but copy the full value. */
  truncate?: boolean;
  /** External explorer link for this value. */
  href?: string;
  /** Allow copy-to-clipboard. */
  copyable?: boolean;
  /** Render the value as plain text (not mono) — e.g. a network name. */
  plain?: boolean;
}

export function DataRow({
  label,
  value,
  truncate = false,
  href,
  copyable = false,
  plain = false,
}: Props) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  async function onCopy() {
    if (await copyText(value)) {
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1600);
    }
  }

  const display = truncate ? truncateMiddle(value) : value;

  return (
    <div className="datarow">
      <span className="datarow-label">{label}</span>
      <span className="datarow-value">
        <span className={plain ? "datarow-text" : "mono datarow-mono"} title={truncate ? value : undefined}>
          {display}
        </span>
        <span className="datarow-actions">
          {copyable && (
            <button
              type="button"
              className="iconbtn"
              onClick={onCopy}
              aria-label={copied ? `${label} copied` : `Copy ${label}`}
            >
              {copied ? <CopiedIcon /> : <CopyIcon />}
            </button>
          )}
          {href && (
            <a
              className="iconbtn"
              href={href}
              target="_blank"
              rel="noreferrer noopener"
              aria-label={`Open ${label} on the explorer`}
            >
              <ExternalIcon />
            </a>
          )}
        </span>
      </span>
    </div>
  );
}

function CopyIcon() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="5.5" y="5.5" width="8" height="8" />
      <path d="M3 10.5 H2.5 V2.5 H10.5 V3" />
    </svg>
  );
}
function CopiedIcon() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 8.5 L6.5 12 L13 4.5" />
    </svg>
  );
}
function ExternalIcon() {
  return (
    <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 2.5 H13.5 V7" />
      <path d="M13.5 2.5 L7.5 8.5" />
      <path d="M11 9 V13 H3 V5 H7" />
    </svg>
  );
}
