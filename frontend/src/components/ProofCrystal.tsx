/*
 * ProofCrystal — Sincerin's signature animated centerpiece (the "anti-globe").
 *
 * One Groth16/BN254 receipt, drawn as a slowly rotating wireframe crystal whose
 * N body facets ARE the N payments in the batch. Each cycle the payments fly in
 * from the perimeter, seal into the crystal (a ring flash), and a single green
 * "verified" pulse blooms from the core — the on-chain settle, the ONE place
 * state-green appears. The crystal never disassembles; it stays a confident,
 * fully-drawn object that happens to breathe.
 *
 * Hand-rolled 3D (Y-rotation + fixed X-tilt + perspective) on a 2D canvas — no
 * WebGL, no deps. Strict B&W hairlines with depth-cued opacity for the dimensional
 * read. DPR-capped, pauses off-screen, and renders a single static assembled
 * frame under prefers-reduced-motion (the object's meaning never needs motion).
 *
 * The real, measured numbers are annotated around it (8.8% of a block's budget,
 * sub-linear in N, 1 on-chain verification) — engraved on the specimen, not in a
 * table. Honest framing: the on-chain cost GROWS with N (31.5M→56.1M), just far
 * slower than the budget — never "constant"/"flat" (see docs/proving-times.md).
 */

import { useEffect, useRef } from "react";

interface Props {
  /** Batch size = number of body facets. Clamped to a legible 6–10. */
  n?: number;
}

const GREEN = "34, 201, 147"; // ~ --state-settled oklch(0.7 0.17 150), the settle hue

export function ProofCrystal({ n = 8 }: Props) {
  const ref = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const N = Math.max(6, Math.min(n, 10));
    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

    // ── model geometry (unit-ish), octagonal crystal: top apex · crown ring ·
    //    body · pavilion ring · bottom apex. The body verticals are the payments.
    const R = 0.92;
    const yU = 0.44;
    const yL = -0.44;
    const yApex = 1.3;
    const ang = (k: number) => (k / N) * Math.PI * 2;
    type V = { x: number; y: number; z: number };
    const apexT: V = { x: 0, y: yApex, z: 0 };
    const apexB: V = { x: 0, y: -yApex, z: 0 };
    const upper: V[] = Array.from({ length: N }, (_, k) => ({ x: R * Math.cos(ang(k)), y: yU, z: R * Math.sin(ang(k)) }));
    const lower: V[] = Array.from({ length: N }, (_, k) => ({ x: R * Math.cos(ang(k)), y: yL, z: R * Math.sin(ang(k)) }));
    const equator: V[] = Array.from({ length: N }, (_, k) => ({ x: R * Math.cos(ang(k)), y: 0, z: R * Math.sin(ang(k)) }));

    type Edge = { a: V; b: V; ring: boolean };
    const edges: Edge[] = [];
    for (let k = 0; k < N; k++) {
      edges.push({ a: apexT, b: upper[k], ring: false }); // crown
      edges.push({ a: upper[k], b: upper[(k + 1) % N], ring: true }); // crown ring
      edges.push({ a: upper[k], b: lower[k], ring: false }); // body facet (a payment)
      edges.push({ a: lower[k], b: lower[(k + 1) % N], ring: true }); // pavilion ring
      edges.push({ a: apexB, b: lower[k], ring: false }); // pavilion
    }

    // ── view transform constants
    const TX = -0.36; // fixed tilt so we look slightly down onto the crown
    const cosTx = Math.cos(TX);
    const sinTx = Math.sin(TX);
    const F = 3.6; // perspective focal distance
    const CYCLE = 6400; // ms per fold-in → seal → verified → rest cycle
    const SPIN = (Math.PI * 2) / 18000; // rad per ms (~18s/turn)

    let w = 0;
    let h = 0;
    let cx = 0;
    let cy = 0;
    let scale = 1;
    let raf = 0;
    let start = 0;
    let running = true;

    const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
    const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
    const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);

    const project = (v: V, a: number) => {
      const ca = Math.cos(a);
      const sa = Math.sin(a);
      const rx = v.x * ca + v.z * sa;
      const rz = -v.x * sa + v.z * ca;
      const ry = v.y;
      const ty = ry * cosTx - rz * sinTx;
      const tz = ry * sinTx + rz * cosTx;
      const s = F / (F - tz);
      return { X: cx + rx * scale * s, Y: cy - ty * scale * s, Z: tz };
    };

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = rect.width;
      h = rect.height;
      cx = w / 2;
      cy = h / 2;
      scale = Math.min(w, h) * 0.34;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    const draw = (a: number, p: number) => {
      ctx.clearRect(0, 0, w, h);
      ctx.lineCap = "round";

      // seal flash factor on the rings, peaking as the payments arrive
      const flash = clamp(1 - Math.abs(p - 0.46) / 0.06, 0, 1);

      for (const e of edges) {
        const A = project(e.a, a);
        const B = project(e.b, a);
        const depth = (A.Z + B.Z) / 2;
        const front = clamp((depth + 1.3) / 2.6, 0, 1);
        let al = 0.16 + 0.78 * front;
        if (e.ring) al = clamp(al + 0.55 * flash, 0, 1);
        ctx.strokeStyle = `rgba(255,255,255,${al.toFixed(3)})`;
        ctx.lineWidth = Math.max(1, scale * 0.009 * (0.55 + 0.85 * front));
        ctx.beginPath();
        ctx.moveTo(A.X, A.Y);
        ctx.lineTo(B.X, B.Y);
        ctx.stroke();
      }

      // crown/pavilion apex nodes (small)
      for (const apex of [apexT, apexB]) {
        const P = project(apex, a);
        ctx.fillStyle = "rgba(255,255,255,0.92)";
        ctx.beginPath();
        ctx.arc(P.X, P.Y, Math.max(1.6, scale * 0.016), 0, Math.PI * 2);
        ctx.fill();
      }

      // payments folding in (perimeter → equator facet), fading out as they seal
      if (p < 0.46) {
        const k0 = easeOut(clamp(p / 0.42, 0, 1));
        for (let k = 0; k < N; k++) {
          const ox = 2.55 * Math.cos(ang(k));
          const oz = 2.55 * Math.sin(ang(k));
          const tgt = equator[k];
          const pos: V = { x: lerp(ox, tgt.x, k0), y: 0, z: lerp(oz, tgt.z, k0) };
          const P = project(pos, a);
          const fadeIn = clamp(p / 0.08, 0, 1);
          const fadeOut = 1 - clamp((p - 0.36) / 0.08, 0, 1);
          const da = 0.9 * fadeIn * fadeOut;
          if (da <= 0.01) continue;
          ctx.fillStyle = `rgba(255,255,255,${da.toFixed(3)})`;
          ctx.beginPath();
          ctx.arc(P.X, P.Y, Math.max(1.6, scale * 0.022), 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // verified — the single green moment: a pulse ring + core spark
      if (p >= 0.44 && p < 0.9) {
        const q = clamp((p - 0.44) / 0.46, 0, 1);
        const rr = scale * (0.16 + 1.05 * easeOut(q));
        ctx.strokeStyle = `rgba(${GREEN},${(0.5 * (1 - q)).toFixed(3)})`;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(cx, cy, rr, 0, Math.PI * 2);
        ctx.stroke();

        const spark = clamp(1 - Math.abs(p - 0.5) / 0.08, 0, 1);
        if (spark > 0) {
          ctx.fillStyle = `rgba(${GREEN},${(0.9 * spark).toFixed(3)})`;
          ctx.beginPath();
          ctx.arc(cx, cy, Math.max(2, scale * 0.03 * spark + 1.5), 0, Math.PI * 2);
          ctx.fill();
        }
      }
    };

    resize();

    if (reduce) {
      // one confident static frame: assembled, mid-turn, the verified ring at rest
      draw(0.7, 0.95);
      const A = project({ x: 0, y: 0, z: 0 }, 0.7);
      ctx.strokeStyle = `rgba(${GREEN},0.42)`;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(A.X, A.Y, scale * 0.62, 0, Math.PI * 2);
      ctx.stroke();
    } else {
      const loop = (now: number) => {
        if (!start) start = now;
        const t = now - start;
        draw(t * SPIN, (t % CYCLE) / CYCLE);
        if (running) raf = requestAnimationFrame(loop);
      };
      raf = requestAnimationFrame(loop);
    }

    // pause off-screen
    const io = new IntersectionObserver(
      (entries) => {
        const vis = entries[0]?.isIntersecting ?? true;
        if (vis && !running && !reduce) {
          running = true;
          raf = requestAnimationFrame((now) => {
            start = now - 0; // resume cleanly
            const loop = (n2: number) => {
              const t = n2 - start;
              draw(t * SPIN, (t % CYCLE) / CYCLE);
              if (running) raf = requestAnimationFrame(loop);
            };
            loop(now);
          });
        } else if (!vis && running) {
          running = false;
          cancelAnimationFrame(raf);
        }
      },
      { threshold: 0.05 },
    );
    io.observe(canvas);

    let resizeT: number | undefined;
    const onResize = () => {
      window.clearTimeout(resizeT);
      resizeT = window.setTimeout(() => {
        resize();
        if (reduce) draw(0.7, 0.95);
      }, 150);
    };
    window.addEventListener("resize", onResize);

    return () => {
      running = false;
      cancelAnimationFrame(raf);
      io.disconnect();
      window.clearTimeout(resizeT);
      window.removeEventListener("resize", onResize);
    };
  }, [n]);

  return (
    <div className="crystal-stage">
      <canvas ref={ref} className="crystal-canvas" aria-hidden="true" />
      <p className="sr-only">
        One Groth16 proof drawn as a rotating crystal: its {n} facets are the {n} payments in
        the batch, sealed into a single receipt and verified on-chain in one transaction.
      </p>
      <ul className="crystal-notes" aria-hidden="true">
        <li className="crystal-note crystal-note--a">
          <span className="crystal-note-n">8.8%</span>
          <span className="crystal-note-l">of a block&rsquo;s budget</span>
        </li>
        <li className="crystal-note crystal-note--b">
          <span className="crystal-note-n">sub-linear</span>
          <span className="crystal-note-l">in N — stays within budget as N grows</span>
        </li>
        <li className="crystal-note crystal-note--c">
          <span className="crystal-note-n">1</span>
          <span className="crystal-note-l">on-chain verification</span>
        </li>
      </ul>
    </div>
  );
}
