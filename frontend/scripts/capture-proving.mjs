/*
 * Capture the s3/02 PROVING screens to prove forward motion (elapsed clock + a
 * moving indeterminate bar). Drives the REAL pipeline via ?previewAddress, then
 * screenshots the proving state at two moments (~10s and ~3min elapsed) so the
 * timer is visibly advanced and the sweep bar has moved between the two frames.
 *
 * Run: NODE_PATH=<npx-playwright-node_modules> node scripts/capture-proving.mjs
 * Env: BASE_URL (default http://localhost:5173), PREVIEW_ADDR, OUT_DIR.
 */
import { chromium } from "playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5173";
const ADDR =
  process.env.PREVIEW_ADDR ??
  "GB5MVC4HEWWBRF7TE3DVVS5F5K7EBJ37UMPKNDGXLL37SDTHLBIBINOL";
const OUT = process.env.OUT_DIR ?? "screenshots";
// The functional pipeline now lives on the /demo surface (s3/02 two-surface split).
const url = `${BASE}/demo?previewAddress=${ADDR}`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function submitAndReachProving(page) {
  await page.goto(url, { waitUntil: "networkidle" });
  // Submit (the previewAddress affordance surfaces the real submit button).
  const submit = page.getByRole("button", { name: /submit withdrawal/i }).first();
  await submit.waitFor({ state: "visible", timeout: 15000 });
  await submit.click();
  // Wait until the status view enters `proving` (the proving meter mounts).
  await page.locator(".provemeter").waitFor({ state: "visible", timeout: 30000 });
  // Let the elapsed clock cross 1s so it never reads 0:00 in any frame.
  await page.locator(".provemeter-clock").waitFor({ state: "visible" });
}

async function readClock(page) {
  return (await page.locator(".provemeter-clock").innerText()).trim();
}

async function readSweepLeft(page) {
  // The on-screen x of the moving highlight's left edge (proves it advanced).
  return page.evaluate(() => {
    const el = document.querySelector(".provemeter-sweep");
    if (!el) return null;
    return Math.round(el.getBoundingClientRect().left * 100) / 100;
  });
}

// Sample the sweep's left edge several times across ~500ms → proves it is
// genuinely animating (a static bar would report one constant value).
async function sampleSweepMotion(page) {
  const xs = [];
  for (let i = 0; i < 6; i++) {
    xs.push(await readSweepLeft(page));
    await sleep(90);
  }
  const uniq = [...new Set(xs)];
  return { xs, moving: uniq.length > 1 };
}

async function run() {
  const browser = await chromium.launch();

  // ── Desktop: one page, two moments (~10s and ~3min) ──────────────────────
  const desktop = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
  });
  const dpage = await desktop.newPage();
  await submitAndReachProving(dpage);

  // Frame A — ~10s elapsed.
  await sleep(10_000);
  const motionA = await sampleSweepMotion(dpage);
  const clockA = await readClock(dpage);
  const sweepA = await readSweepLeft(dpage);
  await dpage.screenshot({ path: `${OUT}/02-status-proving-desktop.png` });
  console.log(`[A ~10s] clock=${clockA} sweepLeft=${sweepA} samples=${motionA.xs.join(",")} moving=${motionA.moving}`);

  // Frame B — ~3min elapsed (clean: recent list has settled down to one row).
  await sleep(168_000); // → ~180s total (10s + 6×0.09s sampling + 168s)
  const motionB = await sampleSweepMotion(dpage);
  const clockB = await readClock(dpage);
  const sweepB = await readSweepLeft(dpage);
  await dpage.screenshot({ path: `${OUT}/02b-status-proving-5min-clean.png` });
  console.log(`[B ~3min] clock=${clockB} sweepLeft=${sweepB} samples=${motionB.xs.join(",")} moving=${motionB.moving}`);

  await desktop.close();

  // ── Mobile: fresh submit, full-page, ~12s elapsed ────────────────────────
  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    isMobile: true,
    hasTouch: true,
  });
  const mpage = await mobile.newPage();
  await submitAndReachProving(mpage);
  await sleep(12_000);
  const clockM = await readClock(mpage);
  await mpage.screenshot({ path: `${OUT}/02-status-proving-mobile.png`, fullPage: true });
  console.log(`[mobile ~12s] clock=${clockM}`);
  await mobile.close();

  await browser.close();

  // ── Assert forward motion (fail loudly if static) ────────────────────────
  const toSec = (mmss) => {
    const [m, s] = mmss.split(":").map(Number);
    return m * 60 + s;
  };
  const secA = toSec(clockA);
  const secB = toSec(clockB);
  console.log(`\nclock advanced: ${clockA} (${secA}s) → ${clockB} (${secB}s)`);
  console.log(`sweep left moved: ${sweepA} → ${sweepB}`);
  if (!(secB > secA + 120)) {
    throw new Error(`elapsed clock did not advance enough: ${clockA} → ${clockB}`);
  }
  if (!motionA.moving || !motionB.moving) {
    throw new Error(
      `indeterminate sweep bar not animating (A=${motionA.xs}, B=${motionB.xs})`,
    );
  }
  console.log("\nOK — timer advanced and the indeterminate bar is animating at both frames.");
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
