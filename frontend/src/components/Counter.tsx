/*
 * Counter — a big display number that counts up the first time it scrolls into
 * view (the payoff/reveal beat from DESIGN.md). The final value is rendered from
 * first paint, so it's correct with JS off, in headless renders, and under
 * reduced-motion; the count-up only ENHANCES an already-correct number.
 */

import { useEffect, useRef, useState } from "react";

interface Props {
  to: number;
  /** ms for the full count-up. */
  duration?: number;
  className?: string;
}

export function Counter({ to, duration = 1100, className = "" }: Props) {
  const ref = useRef<HTMLSpanElement | null>(null);
  const [val, setVal] = useState(to); // correct by default
  const ran = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
    ) {
      return; // stay at final value
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting && !ran.current) {
            ran.current = true;
            setVal(0);
            const start = performance.now();
            const tick = (now: number) => {
              const t = Math.min(1, (now - start) / duration);
              // ease-out-expo
              const eased = t === 1 ? 1 : 1 - Math.pow(2, -10 * t);
              setVal(Math.round(eased * to));
              if (t < 1) requestAnimationFrame(tick);
            };
            requestAnimationFrame(tick);
            io.disconnect();
          }
        }
      },
      { threshold: 0.5 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [to, duration]);

  return (
    <span ref={ref} className={className}>
      {val}
    </span>
  );
}
