/*
 * The live pulse — a monochrome heartbeat (DESIGN.md "the live pulse").
 * Signals the system is alive: on "testnet connected" and during proving.
 * Honours prefers-reduced-motion (the CSS swaps the heartbeat for a steady dot).
 */

interface Props {
  label: string;
  /** "settled" tint for connected/testnet; "proving" tint while working. */
  tone?: "neutral" | "settled" | "proving";
}

export function LivePulse({ label, tone = "neutral" }: Props) {
  return (
    <span className={`livepulse livepulse--${tone}`}>
      <span className="livepulse-dot" aria-hidden="true">
        <span className="livepulse-ring" />
        <span className="livepulse-core" />
      </span>
      <span className="livepulse-label">{label}</span>
    </span>
  );
}
