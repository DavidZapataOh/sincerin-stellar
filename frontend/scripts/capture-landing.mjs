/*
 * Capture the LANDING surface (/) for the s3/02 deliverable.
 *   landing-hero-desktop.png   · the hero fold, 1440
 *   landing-full-desktop.png   · full page, 1440
 *   landing-full-mobile.png    · full page, 390
 * Run: node scripts/capture-landing.mjs   (dev server must be running on BASE_URL)
 */
import { chromium } from "playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5174";
const OUT = process.env.OUT_DIR ?? "screenshots";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function settle(page) {
  // Trigger every Reveal by scrolling to the bottom, then back to top, so the
  // full-page shot shows all sections in their revealed (visible) state.
  await page.evaluate(async () => {
    const step = window.innerHeight;
    for (let y = 0; y <= document.body.scrollHeight; y += step) {
      window.scrollTo(0, y);
      await new Promise((r) => setTimeout(r, 120));
    }
    window.scrollTo(0, 0);
  });
  await sleep(700);
}

async function run() {
  const browser = await chromium.launch();

  // ── Desktop 1440 ─────────────────────────────────────────────────────────
  const desktop = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
  });
  const dp = await desktop.newPage();
  await dp.goto(`${BASE}/`, { waitUntil: "networkidle" });
  await dp.locator("#land-hero-title").waitFor({ state: "visible", timeout: 10000 });

  // hero fold (before scrolling)
  await dp.screenshot({ path: `${OUT}/landing-hero-desktop.png` });
  console.log("✓ landing-hero-desktop.png");

  await settle(dp);
  await dp.screenshot({ path: `${OUT}/landing-full-desktop.png`, fullPage: true });
  console.log("✓ landing-full-desktop.png");

  await desktop.close();

  // ── Mobile 390 ───────────────────────────────────────────────────────────
  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    isMobile: true,
    hasTouch: true,
  });
  const mp = await mobile.newPage();
  await mp.goto(`${BASE}/`, { waitUntil: "networkidle" });
  await mp.locator("#land-hero-title").waitFor({ state: "visible", timeout: 10000 });
  await settle(mp);
  await mp.screenshot({ path: `${OUT}/landing-full-mobile.png`, fullPage: true });
  console.log("✓ landing-full-mobile.png");
  await mobile.close();

  await browser.close();
  console.log("\nDone.");
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
