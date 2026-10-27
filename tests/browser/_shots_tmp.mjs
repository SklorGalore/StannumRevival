import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const APP = "http://localhost:1420";
const OUT = process.env.OUT;
const shim = () => {
  window.__dialogQueue = [];
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (cb) => { const id = Math.floor(Math.random() * 1e9); window[`_${id}`] = cb; return id; },
    invoke: async (cmd, args) => {
      if (cmd === "plugin:dialog|open" || cmd === "plugin:dialog|save") return window.__dialogQueue.shift() ?? null;
      if (cmd.startsWith("plugin:event|")) return 0;
      const r = await fetch(`http://127.0.0.1:1421/invoke/${cmd}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(args ?? {}) });
      const body = await r.json();
      if (!r.ok) throw body;
      return body;
    },
  };
};
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.addInitScript(shim);
await page.goto(APP, { waitUntil: "domcontentloaded" });
const shot = async (name) => { await page.waitForTimeout(400); await page.screenshot({ path: `${OUT}/${name}.png` }); };
const queue = (path) => page.evaluate((p) => window.__dialogQueue.push(p), path);



await queue(process.env.CASE);
await page.getByRole("button", { name: "Open case…" }).first().click();
await page.getByText(/5 buses/).first().waitFor();
await page.getByRole("button", { name: "Records" }).first().click();
await page.getByRole("button", { name: /Switched shunts/ }).click();
await page.getByText("MODSW").first().waitFor();
await page.locator("table tbody tr").first().click();
await page.getByTestId("element-inspector").getByText(/VSWHI/).waitFor();
await shot("switched-shunt-inspector");
await page.getByRole("button", { name: /FACTS devices/ }).click();
await page.getByText("SHMX").first().waitFor();
await page.locator("table tbody tr").first().click();
await page.getByTestId("element-inspector").getByText(/STATCOM_3/).first().waitFor();
await shot("facts-inspector");
await page.getByRole("button", { name: /^Buses/ }).click();
await page.locator("table tbody tr").nth(4).click();
await page.getByTestId("element-inspector").getByText(/Switched shunt/).first().waitFor();
await shot("bus-connections");
console.log(errors.length ? errors : "ok");
await browser.close();
