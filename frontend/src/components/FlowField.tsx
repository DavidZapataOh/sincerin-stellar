/*
 * FlowField — a subtle on-brand motif for the inverted-black sections.
 *
 * Drifting hairline nodes that slowly stream left→right and occasionally link,
 * echoing the aggregation idea (many things moving toward one) WITHOUT competing
 * with the foreground type. Monochrome, very low contrast, decorative-only
 * (aria-hidden). Canvas so it stays cheap; capped DPR; pauses off-screen.
 *
 * prefers-reduced-motion → renders one static frame (no rAF loop). The section
 * never depends on it; it's pure atmosphere behind already-legible content.
 */

import { useEffect, useRef } from "react";

interface Props {
  /** Particle density multiplier. */
  density?: number;
}

export function FlowField({ density = 1 }: Props) {
  const ref = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

    let raf = 0;
    let w = 0;
    let h = 0;
    let dpr = 1;
    type P = { x: number; y: number; vx: number; vy: number; r: number };
    let nodes: P[] = [];

    const seed = () => {
      const area = w * h;
      const target = Math.round((area / 26000) * density);
      nodes = Array.from({ length: Math.max(14, Math.min(target, 90)) }, () => ({
        x: Math.random() * w,
        y: Math.random() * h,
        // gentle rightward drift (toward the "one" on the right), tiny vertical wander
        vx: 0.12 + Math.random() * 0.34,
        vy: (Math.random() - 0.5) * 0.12,
        r: 0.8 + Math.random() * 1.6,
      }));
    };

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = rect.width;
      h = rect.height;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      seed();
    };

    const LINK = 124; // px distance to draw a hairline between two nodes

    const draw = () => {
      ctx.clearRect(0, 0, w, h);
      // links first (under the dots)
      ctx.lineWidth = 1;
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i];
          const b = nodes[j];
          const dx = a.x - b.x;
          const dy = a.y - b.y;
          const d2 = dx * dx + dy * dy;
          if (d2 < LINK * LINK) {
            const t = 1 - Math.sqrt(d2) / LINK;
            ctx.strokeStyle = `rgba(255,255,255,${(t * 0.1).toFixed(3)})`;
            ctx.beginPath();
            ctx.moveTo(a.x, a.y);
            ctx.lineTo(b.x, b.y);
            ctx.stroke();
          }
        }
      }
      // dots
      for (const p of nodes) {
        ctx.fillStyle = "rgba(255,255,255,0.28)";
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fill();
      }
    };

    const step = () => {
      for (const p of nodes) {
        p.x += p.vx;
        p.y += p.vy;
        if (p.x > w + 8) {
          p.x = -8;
          p.y = Math.random() * h;
        }
        if (p.y < -8) p.y = h + 8;
        if (p.y > h + 8) p.y = -8;
      }
      draw();
      raf = requestAnimationFrame(step);
    };

    resize();
    if (reduce) {
      draw(); // one static frame
    } else {
      raf = requestAnimationFrame(step);
    }

    let resizeT: number | undefined;
    const onResize = () => {
      window.clearTimeout(resizeT);
      resizeT = window.setTimeout(resize, 150);
    };
    window.addEventListener("resize", onResize);

    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(resizeT);
      window.removeEventListener("resize", onResize);
    };
  }, [density]);

  return <canvas ref={ref} className="flowfield" aria-hidden="true" />;
}
