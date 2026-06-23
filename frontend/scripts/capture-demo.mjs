/*
 * Capture the DEMO surface (/demo) for the s3/02 deliverable. Drives the REAL
 * sequencer pipeline via ?previewAddress (the submit button surfaces with a
 * recipient set), then screenshots:
 *   demo-submit-desktop.png / -mobile.png   · the connected submit view
 *   demo-proving-desktop.png / -mobile.png  · the proving state (live mm:ss + bar)
 *   demo-settled-desktop.png / -mobile.png  · the settled Result view
 *
 * STUB DISCLOSURE: the settled view is rendered via ?previewSettled=<hash> using
 * the REAL historic on-chain tx (aedc1cc4…) — a render-only affordance, because
 * a fresh live on-chain settle is the orchestrator's gate (needs GPU/stellar
 * CLI signing), not the frontend's. The hash + explorer link are real; nothing
 * is fabricated. Submit + proving exercise the genuine sequencer (real receipt,
 * FIXTURE_PROVE_DELAY before the real settle).
 */
import { chromium } from "playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5174";
const ADDR =
  process.env.PREVIEW_ADDR ??
  "GB5MVC4HEWWBRF7TE3DVVS5F5K7EBJ37UMPKNDGXLL37SDTHLBIBINOL";
const SETTLED_TX =
  process.env.SETTLED_TX ??
  "aedc1cc42f112d65913d4b1b5fb0e9b5636481e2f10a86f85ed21f5c0f605ea9";
const OUT = process.env.OUT_DIR ?? "screenshots";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const desktopVP = { width: 1440, height: 900, deviceScaleFactor: 2 };
const mobileVP = {
  width: 390,
  height: 844,
  deviceScaleFactor: 2,
  isMobile: true,
  hasTouch: true,
};

async function captureSubmit(ctx, label, full) {
  const page = await ctx.newPage();
  await page.goto(`${BASE}/demo?previewAddress=${ADDR}`, { waitUntil: "networkidle" });
  await page.getByRole("button", { name: /submit withdrawal/i }).first()
    .waitFor({ state: "visible", timeout: 15000 });
  await sleep(400);
  await page.screenshot({ path: `${OUT}/demo-submit-${label}.png`, fullPage: full });
  console.log(`✓ demo-submit-${label}.png`);
  await page.close();
}

async function captureProving(ctx, label) {
  const page = await ctx.newPage();
  await page.goto(`${BASE}/demo?previewAddress=${ADDR}`, { waitUntil: "networkidle" });
  const submit = page.getByRole("button", { name: /submit withdrawal/i }).first();
  await submit.waitFor({ state: "visible", timeout: 15000 });
  await submit.click();
  await page.locator(".provemeter").waitFor({ state: "visible", timeout: 30000 });
  // let the clock advance to a non-zero, then screenshot
  await sleep(11_000);
  const clock = (await page.locator(".provemeter-clock").innerText()).trim();
  await page.screenshot({ path: `${OUT}/demo-proving-${label}.png` });
  console.log(`✓ demo-proving-${label}.png (clock=${clock})`);
  await page.close();
  return clock;
}

async function captureSettled(ctx, label, full) {
  const page = await ctx.newPage();
  await page.goto(
    `${BASE}/demo?previewAddress=${ADDR}&previewSettled=${SETTLED_TX}`,
    { waitUntil: "networkidle" },
  );
  await page.locator("#result-title").waitFor({ state: "visible", timeout: 15000 });
  await sleep(900); // let the settle reveal + count finish
  await page.screenshot({ path: `${OUT}/demo-settled-${label}.png`, fullPage: full });
  console.log(`✓ demo-settled-${label}.png`);
  await page.close();
}

async function run() {
  const browser = await chromium.launch();

  const desktop = await browser.newContext({ viewport: desktopVP, deviceScaleFactor: 2 });
  await captureSubmit(desktop, "desktop", false);
  await captureSettled(desktop, "desktop", false);
  await captureSettled(desktop, "desktop-full", true);
  const clockD = await captureProving(desktop, "desktop");
  await desktop.close();

  const mobile = await browser.newContext(mobileVP);
  await captureSubmit(mobile, "mobile", true);
  await captureSettled(mobile, "mobile", true);
  await captureProving(mobile, "mobile");
  await mobile.close();

  await browser.close();
  console.log(`\nDone. (desktop proving clock=${clockD})`);
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
