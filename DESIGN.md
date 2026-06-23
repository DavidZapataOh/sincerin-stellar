# Design

> Visual system for **Sincerin** — the demo UI of the Confidential Payments Rollup.
> Register: **brand** (design IS the product). North: *"this is REAL infra, zero mocks."*
> Lane (from references Succinct · zkVerify · web3privacy): **monochrome, high-contrast,
> sharp, confident display type — one disciplined system.** Explicitly **NOT a terminal**
> (no green-on-black, no mono-as-costume). Distinct from each ref, not a clone.

## Theme

**Black & white, high-contrast, editorial-bold.** Default surface is **pure white**; deliberate **full-black inverted sections** carry the emphasis beats (hero, the settled/"real" moment). Sharp corners (0px). Big confident typography. Generous whitespace. Color is **not** the brand carrier — **contrast, type, motion, and inversion are.** A single functional accent appears **only** at the on-chain payoff (settled) and live signals; everything else is monochrome.

## Color — OKLCH (true neutral, chroma 0)

```css
--bg:          oklch(1 0 0);        /* pure white — default canvas (Succinct/zkVerify lane) */
--bg-invert:   oklch(0.145 0 0);    /* near-black — inverted sections: hero, settled, "real" beats */
--ink:         oklch(0.17 0 0);     /* primary text on white — ≥ 4.5:1 ✓ */
--ink-invert:  oklch(0.985 0 0);    /* primary text on black */
--muted:       oklch(0.50 0 0);     /* secondary text on white — 4.5:1 ✓ (no light-gray slop) */
--muted-invert:oklch(0.72 0 0);     /* secondary text on black */
--line:        oklch(0.91 0 0);     /* hairline rules / borders on white */
--line-invert: oklch(0.28 0 0);     /* hairline on black */
--surface:     oklch(0.975 0 0);    /* faint raised surface on white (used sparingly, not card-spam) */
```

**Functional state colors — the ONLY non-neutral hues, semantic-only, minimal.** Never brand decoration; always paired with **icon + label** so color is never load-bearing (WCAG / colorblind-safe).

```css
--state-settled: oklch(0.70 0.17 150);  /* settled on-chain — the single "real payoff" pop + live testnet pulse */
--state-proving: oklch(0.74 0.16 75);   /* proving / in-flight — the working state */
--state-failed:  oklch(0.58 0.22 27);   /* failed — clear, never silent */
```

State is communicated by **icon + label + motion + position** first; the hue is reinforcement. In strict-mono contexts, states still read without color.

## Typography

Distinctive, **non-reflex** (no Inter / Space Grotesk / DM / Plex / Geist). Three roles on a real contrast axis:

```css
--font-display: "Clash Display", "Archivo", sans-serif;  /* hero numbers, big statements, the "8 → 1" */
--font-text:    "General Sans", "Archivo", system-ui, sans-serif;  /* body, UI, labels */
--font-data:    "Martian Mono", ui-monospace, monospace;  /* hashes, amounts, instr counts, addresses */
```

- **Display** (Clash Display, Fontshare): confident, geometric-with-character; carries the brand voice in big monochrome statements. Display heading clamp max ≤ 6rem; letter-spacing ≥ -0.04em; `text-wrap: balance`.
- **Text** (General Sans): clean, neutral, highly readable; the workhorse. Body line length 65–75ch.
- **Data** (Martian Mono): mono **only for genuine data** (tx hashes, XLM amounts, instr counts, G-addresses, cycle counts) — earns its place because the data IS technical; never mono for prose (that's the costume the bans warn about).
- Scale: modular, ratio ≥ 1.25, fluid `clamp()` for headings.

## Layout & Spacing

- Base unit **8px** (Succinct/zkVerify). Fluid `clamp()` spacing that breathes on large viewports; vary rhythm (generous separations, tight groupings).
- **Sharp corners: `border-radius: 0`** on cards, inputs, buttons (precise/technical; matches Succinct + zkVerify). Pills allowed only for status chips.
- Editorial, confident grid; **intentional asymmetry** for emphasis. Hairline rules (`--line`) to structure, not boxes-everywhere. Cards only when truly the right affordance; **never nested cards**.
- Semantic z-index scale (dropdown → sticky → modal-backdrop → modal → toast → tooltip).

## Components

- **Buttons:** primary = solid near-black on white (or inverted: white on black in dark sections), sharp, bold label. Secondary = hairline-bordered, transparent. No shadows by default. Clear focus ring (keyboard).
- **Inputs:** transparent bg, hairline border, sharp, generous padding; placeholder at full 4.5:1 (not light-gray).
- **Status chip** (pending/proving/settled/failed): icon + label + state hue; the one place a pill radius is allowed.
- **Data row:** label (text) + value (mono), hairline-separated; for hashes/amounts/addresses with a copy + explorer affordance.
- **The aggregation visual** (hero imagery): a custom **SVG/canvas** of "N withdrawals → 1 proof → 1 tx" — this is the centerpiece "imagery" (brand register implies imagery; here it's generated data-viz, not stock photos). Monochrome, precise, animated on the settle.

## Motion

- Intentional, **ease-out** (quart/quint/expo), no bounce/elastic. Motion is part of the build, not an afterthought.
- **The live pulse:** a monochrome heartbeat on "testnet ● connected" and during `proving` — signals the system is alive and working (carries the "real" message over the ~5-min proving wait without a dead spinner).
- **Counting/reveal:** big numbers (the "8 → 1", the instr count, the credited amount) animate on settle — the payoff beat, with the one allowed `--state-settled` pop + a brief black-inversion flash.
- **`prefers-reduced-motion`:** every animation has a crossfade/instant alternative. Reveals enhance an already-visible default (never gate content on a transition).

## Imagery

Brand register → imagery required, but here imagery = **generated technical visuals**: the aggregation SVG (N→1), the two-axis benchmark chart (from `bench/`), the live batch/epoch state. No stock photos, no colored-`<div>` placeholders. One decisive hero visual (the aggregation), not five mediocre ones.

## Bans (enforced)

No terminal/green-on-black aesthetic · no mono-as-costume (mono = data only) · no side-stripe borders · no gradient text · no glassmorphism-by-default · no hero-metric SaaS template · no identical card grids · no all-caps tracked eyebrow over every section · no numbered `01/02/03` section scaffolding · no text overflow at any breakpoint · no cream/sand body bg.
