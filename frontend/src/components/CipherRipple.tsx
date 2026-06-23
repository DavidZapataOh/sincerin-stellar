/*
 * CipherRipple — hero art option 3. A fine lattice of cells with diagonal waves
 * of "scramble" sweeping across: cells in the crest flip to glyphs, the troughs
 * fade to faint dots — like an encryption pass washing over data. Hypnotic,
 * geometric, sharp. Symbolises confidential / ciphered. Monochrome on black.
 */

import { useEffect, useRef } from "react";

export function CipherRipple() {
  const ref = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

    let w = 0;
    let h = 0;
    let raf = 0;
    let cell = 26;
    let cols = 0;
    let rows = 0;
    let offX = 0;
    let offY = 0;

    const resize = () => {
      const r = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = r.width;
      h = r.height;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      cell = Math.max(20, Math.min(w, h) * 0.052);
      cols = Math.ceil(w / cell) + 1;
      rows = Math.ceil(h / cell) + 1;
      offX = (w - cols * cell) / 2 + cell / 2;
      offY = (h - rows * cell) / 2 + cell / 2;
    };

    const render = (t: number) => {
      ctx.clearRect(0, 0, w, h);
      const a = t * 0.0016;
      const g = cell * 0.28; // glyph half-size
      ctx.lineCap = "round";
      for (let j = 0; j < rows; j++) {
        for (let i = 0; i < cols; i++) {
          const cx = offX + i * cell;
          const cy = offY + j * cell;
          // two diagonal waves crossing → a richer travelling interference
          const v =
            0.6 * Math.sin(i * 0.62 + j * 0.44 - a) +
            0.4 * Math.sin(i * 0.31 - j * 0.52 - a * 0.7);
          const lit = (v + 1) / 2; // 0..1
          if (lit > 0.62) {
            // active: a short diagonal "cipher" stroke, direction by parity
            const al = Math.min(1, (lit - 0.62) / 0.3) * 0.9;
            ctx.strokeStyle = `rgba(255,255,255,${al.toFixed(3)})`;
            ctx.lineWidth = 1.6;
            ctx.beginPath();
            if ((i + j) % 2 === 0) {
              ctx.moveTo(cx - g, cy - g);
              ctx.lineTo(cx + g, cy + g);
            } else {
              ctx.moveTo(cx + g, cy - g);
              ctx.lineTo(cx - g, cy + g);
            }
            ctx.stroke();
          } else {
            // dormant: a faint dot
            const al = 0.06 + 0.12 * lit;
            ctx.fillStyle = `rgba(255,255,255,${al.toFixed(3)})`;
            ctx.beginPath();
            ctx.arc(cx, cy, 1.3, 0, Math.PI * 2);
            ctx.fill();
          }
        }
      }
      if (!reduce) raf = requestAnimationFrame(render);
    };

    resize();
    if (reduce) render(0);
    else raf = requestAnimationFrame(render);

    let rt: number | undefined;
    const onResize = () => {
      window.clearTimeout(rt);
      rt = window.setTimeout(() => {
        resize();
        if (reduce) render(0);
      }, 150);
    };
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(rt);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return <canvas ref={ref} className="hero-art" aria-hidden="true" />;
}
