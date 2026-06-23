/*
 * Reveal — a scroll-into-view entrance that ENHANCES an already-visible default.
 *
 * The content is fully visible from first paint (opacity 1); the component only
 * adds a `data-reveal` hook that the CSS uses to play a one-time rise+fade WHEN
 * the element enters the viewport. Critically: if IntersectionObserver never
 * fires (headless render, hidden tab, reduced-motion, JS-less), the content is
 * already there — never gated behind a transition (impeccable: "reveals enhance
 * an already-visible default"). `prefers-reduced-motion` is handled in CSS.
 */

import { useEffect, useRef, useState, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  className?: string;
  /** Stagger delay in ms (for sequenced cards in a row). */
  delay?: number;
  as?: "div" | "section" | "li";
}

export function Reveal({ children, className = "", delay = 0, as = "div" }: Props) {
  const ref = useRef<HTMLElement | null>(null);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    // Respect reduced-motion: mark shown immediately, no observer.
    if (
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
    ) {
      setShown(true);
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            setShown(true);
            io.disconnect();
          }
        }
      },
      { rootMargin: "0px 0px -10% 0px", threshold: 0.05 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  const Tag = as;
  return (
    <Tag
      ref={ref as never}
      className={`reveal ${className}`}
      data-shown={shown ? "true" : "false"}
      style={delay ? { transitionDelay: `${delay}ms` } : undefined}
    >
      {children}
    </Tag>
  );
}
