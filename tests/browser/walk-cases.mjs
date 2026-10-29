// Real-case walkthrough: opens each case and takes it through every module,
// with a screenshot at each step, so a release can be checked on full-size
// cases. Unlike smoke.mjs it asserts nothing; it reports each step as OK or
// FAIL and keeps going.
//
//   cargo run --release -p stannum-core --example dev_bridge &   # :1421
//   npm run build && npx vite preview --port 1420 --strictPort &
//   STANNUM_SHOTS=shots node tests/browser/walk-cases.mjs tests/large/SyntheticUSA.RAW crates/nodal/Texas2k_series25_case1_summerpeak.RAW
//
// Steps: open, AC solve, each results tab, Branches by loading, sensitivities,
// N-1 contingencies (screen, then AC for the worst 50), Records, a one-line
// around the most loaded branch, the network and map views, then a Records
// edit compared with the file in Case diff (RAW 33 and 35 only). Set
// STANNUM_WALK_SKIP to a comma list of step names to leave out, such as
// "contingencies" (minutes on 80,000 buses).
import { createRequire } from "node:module";
import { basename, resolve } from "node:path";
import { mkdirSync } from "node:fs";
// Resolved through require() so a globally installed Playwright works via NODE_PATH.
const { chromium } = createRequire(import.meta.url)("playwright");

const APP = "http://localhost:1420";
const SHOTS = process.env.STANNUM_SHOTS;
const SKIP = (process.env.STANNUM_WALK_SKIP ?? "").split(",").filter(Boolean);
const cases = process.argv.slice(2).map((p) => resolve(p));
if (!cases.length) {
  console.error("Usage: node tests/browser/walk-cases.mjs <case file>...");
  process.exit(2);
}
if (SHOTS) mkdirSync(SHOTS, { recursive: true });

// Same shim as smoke.mjs: Tauri's IPC goes to the dev bridge over HTTP.
const shim = () => {
  window.__dialogQueue = [];
  window.__files = {};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (cb) => {
      const id = Math.floor(Math.random() * 1e9);
      window[`_${id}`] = cb;
      return id;
    },
    invoke: async (cmd, args) => {
      if (cmd === "plugin:dialog|open" || cmd === "plugin:dialog|save") return window.__dialogQueue.shift() ?? null;
      if (cmd === "save_diagram") return void (window.__files[args.path] = args.content);
      if (cmd === "save_binary") return void (window.__files[args.path] = atob(args.dataB64));
      if (cmd === "load_diagram") return window.__files[args.path];
      if (cmd.startsWith("plugin:event|")) return 0;
      const r = await fetch(`http://127.0.0.1:1421/invoke/${cmd}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(args ?? {}),
      });
      const body = await r.json();
      if (!r.ok) throw body;
      return body;
    },
  };
};

const browser = await chromium.launch();
let failures = 0;
for (const path of cases) {
  const name = basename(path).replace(/\.[^.]+$/, "");
  console.log(`=== ${name}`);
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.addInitScript(shim);
  await page.goto(APP, { waitUntil: "domcontentloaded" });

  let n = 0;
  const shot = async (what) => {
    if (!SHOTS) return;
    await page.waitForTimeout(500);
    await page.screenshot({ path: `${SHOTS}/${name}-${String(++n).padStart(2, "0")}-${what}.png` });
  };
  const rail = (label) => page.getByRole("button", { name: label, exact: true }).first().click();
  const tab = (label) => page.getByRole("tab", { name: label }).first().click();
  const visible = (text) => page.locator("button:visible", { hasText: text }).first();
  // Large cases take minutes to screen and seconds to lay out.
  const T = Number(process.env.STANNUM_WALK_TIMEOUT ?? 40 * 60_000);
  const settle = (ms) => page.waitForTimeout(ms);
  const step = async (what, fn) => {
    if (SKIP.includes(what)) return;
    const t = Date.now();
    try {
      await fn();
      console.log(`OK   ${what} (${((Date.now() - t) / 1000).toFixed(1)} s)`);
    } catch (e) {
      failures++;
      console.log(`FAIL ${what}: ${String(e.message).split("\n")[0]}`);
      await shot(`fail-${what.replace(/\W+/g, "-")}`);
    }
  };

  await step("open", async () => {
    await page.evaluate((p) => window.__dialogQueue.push(p), path);
    await page.getByRole("button", { name: /Open (case|another)…/ }).first().click();
    await page.getByText(/ buses ·/).first().waitFor({ timeout: T });
    await shot("opened");
  });
  await step("solve", async () => {
    await rail("Power flow");
    await page.getByRole("button", { name: "Solve", exact: true }).click();
    await page.getByText(/Converged|did not converge|Iteration limit/i).first().waitFor({ timeout: T });
    await settle(1500);
    await shot("overview");
  });
  for (const t of ["Buses", "Generators", "Branches", "DC lines", "DC network", "Areas", "FACTS"]) {
    if (!(await page.getByRole("tab", { name: new RegExp(`^${t}`) }).count())) continue;
    await step(t.toLowerCase(), async () => {
      await tab(new RegExp(`^${t}`));
      await settle(2500);
      await shot(t.toLowerCase().replace(" ", "-"));
    });
  }
  await step("loading", async () => {
    await tab(/^Branches/);
    const header = page.locator("th:visible", { hasText: "Loading" }).first();
    await header.click(); // low to high
    await settle(1500);
    await header.click(); // high to low
    await settle(2500);
    await shot("branches-by-loading");
  });
  await step("sensitivities", async () => {
    await page.getByRole("button", { name: /Calculate sensitivities/ }).click();
    await page.getByText(/Calculated sensitivities|Calculating sensitivities failed/).first().waitFor({ timeout: T });
    await tab(/^Sensitivities/);
    await settle(3000);
    await shot("sensitivities");
  });
  await step("contingencies", async () => {
    await tab(/^Contingencies/);
    await page.getByTestId("run-contingencies").click();
    await settle(2000);
    await page.getByTestId("contingency-progress").waitFor({ state: "detached", timeout: T });
    await settle(2000);
    await shot("contingencies");
  });
  await step("records", async () => {
    await rail("Records");
    await page.getByText(/^[\d,]+ rows$/).filter({ visible: true }).first().waitFor({ timeout: T });
    await settle(1500);
    await shot("records");
  });
  await step("one-line", async () => {
    await rail("Power flow");
    await tab(/^Overview/);
    await page.locator("tr", { hasText: "Branch rating A" }).locator('button[aria-label^="Draw a one-line around"]').first().click();
    await settle(1500);
    await visible(/^One-line$/).click();
    await settle(8000);
    await shot("one-line");
  });
  await step("network", async () => {
    await visible(/^Network$/).click();
    await settle(8000);
    await shot("network");
  });
  await step("map", async () => {
    await visible(/^Map$/).click();
    await settle(8000);
    await shot("map");
  });
  await step("case diff", async () => {
    await rail("Records");
    await settle(1500);
    if (await page.getByText(/power flow model ·/).filter({ visible: true }).count()) {
      console.log("     (Records shows the power flow model; Case diff needs RAW 33 or 35)");
      return;
    }
    await page.getByRole("button", { name: /^Loads/ }).first().click();
    await settle(3000);
    const cell = page.locator("table:visible tbody tr:not([data-spacer]):not([data-loading])").first().locator("td").nth(6);
    await cell.dblclick();
    await page.keyboard.press("Control+a");
    await page.keyboard.type("123.5");
    await page.keyboard.press("Enter");
    await page.getByRole("button", { name: "Apply", exact: true }).first().click();
    await settle(3000);
    await rail("Case diff");
    await visible(/Compare with its file/).click();
    await page.getByText(/differing record|failed/).filter({ visible: true }).first().waitFor({ timeout: T });
    await settle(2000);
    await page.locator("tbody tr:visible", { hasText: /changed/i }).first().click();
    await settle(1500);
    await shot("case-diff");
  });
  if (errors.length) console.log("page errors:", errors);
  await page.close();
}
await browser.close();
process.exit(failures ? 1 : 0);
