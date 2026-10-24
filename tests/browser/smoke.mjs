// Browser smoke test: drives the real UI against the real Rust core.
//
//   cargo run --release -p stannum-core --example dev_bridge &   # :1421
//   npx vite --port 1420 &
//   NODE_PATH=$(npm root -g) STANNUM_MATPOWER=case9.m STANNUM_RAW=a.raw STANNUM_RAW_B=b.raw node tests/browser/smoke.mjs
//
// Tauri's IPC is replaced by a shim that forwards commands to the bridge and
// answers file dialogs from a queue. Screenshots go to $STANNUM_SHOTS if set.
import { createRequire } from "node:module";
// Resolved through require() so a globally installed Playwright works via NODE_PATH.
const { chromium } = createRequire(import.meta.url)("playwright");
import assert from "node:assert/strict";

const { STANNUM_MATPOWER, STANNUM_RAW, STANNUM_RAW_B, STANNUM_SHOTS } = process.env;
const APP = process.env.STANNUM_URL ?? "http://localhost:1420";
assert(STANNUM_MATPOWER && STANNUM_RAW && STANNUM_RAW_B, "set STANNUM_MATPOWER, STANNUM_RAW and STANNUM_RAW_B");

const shim = () => {
  window.__dialogQueue = [];
  // Event listeners are never registered here, so unlisten is a no-op.
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
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.addInitScript(shim);
await page.goto(APP, { waitUntil: "domcontentloaded" });
const shot = async (name) => {
  if (!STANNUM_SHOTS) return;
  await page.waitForTimeout(300); // let hover/transition styles settle
  await page.screenshot({ path: `${STANNUM_SHOTS}/${name}.png` });
};
const queue = (path) => page.evaluate((p) => window.__dialogQueue.push(p), path);

// Power flow on a MATPOWER case.
await queue(STANNUM_MATPOWER);
await page.getByRole("button", { name: "Open case…" }).first().click();
await page.getByRole("button", { name: "Solve", exact: true }).click();
await page.getByText("Converged").first().waitFor();
await page.getByText("Operating limits").waitFor();
await shot("power-overview");
await page.getByRole("tab", { name: /Branches/ }).click();
await page.getByText("Loading (%)").waitFor();
await shot("power-branches");
await page.getByRole("button", { name: "Calculate sensitivities" }).click();
await page.getByText("How to read these results").waitFor();
await page.getByText(/Response \(/).waitFor();
await shot("power-sensitivities");

// Topology on a RAW v35 case.
await queue(STANNUM_RAW);
await page.getByRole("button", { name: "Open another…" }).click();
await page.getByRole("button", { name: "Topology", exact: true }).click();
await page.getByText("Articulation buses").first().waitFor();
await page.locator("canvas").first().waitFor();
const firstBus = await page.locator("table tbody tr td").first().innerText();
const busNumbers = await page.locator("table tbody tr td:first-child").allInnerTexts();
await page.getByLabel("From bus").fill(firstBus);
await page.getByLabel("To bus").fill(busNumbers[Math.min(5, busNumbers.length - 1)]);
await page.getByRole("button", { name: "Path", exact: true }).click();
await page.getByTestId("path-result").filter({ hasText: /hop|No active AC path/ }).waitFor();
await shot("topology");

// The same RAW case solves too.
await page.getByRole("button", { name: "Power flow", exact: true }).click();
await page.getByRole("button", { name: "Solve", exact: true }).click();
await page.getByText(/Converged|Iteration limit|Q limits infeasible/).first().waitFor();

// Case diff, with the open case as baseline.
await page.getByRole("button", { name: "Case diff", exact: true }).click();
await queue(STANNUM_RAW_B);
await page.getByRole("button", { name: "Browse…" }).nth(1).click();
await page.getByRole("button", { name: /Compare files/ }).click();
await page.getByText(/differing records|No differences/).first().waitFor();
const firstRecord = page.locator("section:has-text('Records') table tbody tr").first();
if (await firstRecord.count()) {
  await firstRecord.click();
  await page.getByText("Original lines").waitFor();
}
await shot("case-diff");

// A one-line generated from the open case, with the power flow results
// solved above drawn on it.
await page.getByRole("button", { name: "Diagram", exact: true }).click();
await page.getByRole("button", { name: "From case" }).click();
await page.getByRole("button", { name: "Generate", exact: true }).click();
await page.getByRole("dialog").waitFor({ state: "detached" });
await page.locator(".react-flow__node-bus").first().waitFor();
assert.equal(await page.locator(".react-flow__node-bus").count(), 5);
await page.getByText(/\d\.\d{3} pu/).first().waitFor();
await page.waitForTimeout(400); // fit-to-screen animation
await shot("diagram");

// Print style, then undo after deleting everything.
await page.getByRole("button", { name: "Print style" }).click();
await shot("diagram-print");
await page.getByRole("button", { name: "Print style" }).click();
const nodeCount = await page.locator(".react-flow__node").count();
await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
await page.keyboard.press("Control+a");
await page.keyboard.press("Delete");
assert.equal(await page.locator(".react-flow__node").count(), 0);
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__node").count(), nodeCount);

// A local drawing one hop around bus 4: the studied bus is highlighted and
// branches leaving the drawing end in markers. Double-clicking a marker brings
// that bus in without moving what is already drawn.
await page.getByRole("button", { name: "From case" }).click();
await page.getByLabel(/Around a bus/).check();
await page.getByRole("combobox", { name: "Bus number" }).fill("4");
await page.getByLabel("Hops out").selectOption("1");
page.once("dialog", (d) => d.accept()); // replace the edited drawing
await page.getByRole("button", { name: "Generate", exact: true }).click();
await page.getByRole("dialog").waitFor({ state: "detached" });
await page.getByText("Around").waitFor();
assert.equal(await page.locator(".react-flow__node-bus").count(), 3, "bus 4 and its neighbours 2 and 5");
const studied = page.locator(".react-flow__node-bus").filter({ has: page.locator("title", { hasText: "Studied bus" }) });
assert.equal(await studied.count(), 1, "studied bus highlighted");
const markers = page.locator(".react-flow__node-offPage");
assert.equal(await markers.count(), 2, "markers toward buses 1 and 3");
await page.waitForTimeout(400);
const studiedAt = await page.evaluate(() => document.querySelector('[data-id="b4"]').style.transform);
await markers.first().dblclick();
await page.waitForFunction(() => document.querySelectorAll(".react-flow__node-bus").length === 4);
assert.equal(await page.evaluate(() => document.querySelector('[data-id="b4"]').style.transform), studiedAt, "expand moved bus 4");
await page.waitForTimeout(500); // wires go in a frame after the taps
await shot("diagram-local");
await page.keyboard.press("Escape");
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__node-bus").count(), 3, "undo removes the expansion");

// Hand drawing: a wire released on a bus bar, between its taps, gets a new tap.
page.once("dialog", (d) => d.accept());
await page.getByRole("button", { name: "New", exact: true }).click();
const pane = page.locator(".react-flow__pane");
await page.locator("[draggable=true]").filter({ hasText: /^Bus$/ }).dragTo(pane, { targetPosition: { x: 450, y: 200 } });
await page.locator("[draggable=true]").filter({ hasText: "Load (Static)" }).dragTo(pane, { targetPosition: { x: 330, y: 400 } });
const load = page.locator(".react-flow__node-load");
await load.hover();
const terminal = await load.locator(".react-flow__handle").boundingBox();
const bar = await page.locator(".react-flow__node-bus").boundingBox();
await page.mouse.move(terminal.x + terminal.width / 2, terminal.y + terminal.height / 2);
await page.mouse.down();
await page.mouse.move(bar.x + 61, bar.y + bar.height + 4, { steps: 10 });
await page.mouse.up();
await page.locator(".react-flow__edge").first().waitFor();
await shot("diagram-drawn");
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__edge").count(), 0, "undo removes the wire");

// Inactive modules must not show the diagram through them.
await page.getByRole("button", { name: "Power flow", exact: true }).click();
const showing = await page.evaluate(
  () => [...document.querySelectorAll(".react-flow__node")].filter((n) => getComputedStyle(n).visibility === "visible").length
);
assert.equal(showing, 0, "diagram symbols visible on another module");

assert.deepEqual(errors, [], `page errors: ${errors.join("; ")}`);
await browser.close();
console.log("smoke test passed");
