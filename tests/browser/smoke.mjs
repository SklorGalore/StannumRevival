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

const { STANNUM_MATPOWER, STANNUM_RAW, STANNUM_RAW_B, STANNUM_RAW_DC, STANNUM_DC_A, STANNUM_DC_B, STANNUM_SHOTS } = process.env;
const APP = process.env.STANNUM_URL ?? "http://localhost:1420";
assert(STANNUM_MATPOWER && STANNUM_RAW && STANNUM_RAW_B, "set STANNUM_MATPOWER, STANNUM_RAW and STANNUM_RAW_B");

const shim = () => {
  window.__dialogQueue = [];
  window.__files = {};
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
      // Saved and exported files stay in the page, where the test reads them.
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

// Filters and colour rules narrow the bus table and draw a legend.
await page.getByRole("tab", { name: /Filters & colours/ }).click();
await page.getByLabel("Colour buses by").selectOption("base_kv");
await page.getByTestId("graph-legend").waitFor();
await page.getByRole("button", { name: "Add filter" }).click();
await page.getByLabel("Filter field").selectOption("number");
await page.getByLabel("Filter operator").selectOption("eq");
await page.getByLabel("Filter value").fill(firstBus);
await page.getByTestId("filter-counts").filter({ hasText: /Showing 1 of/ }).waitFor();
await page.getByRole("button", { name: "Clear all" }).click();
await page.getByLabel("Colour buses by").selectOption("");
await page.getByRole("tab", { name: "Selection" }).click();

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

// One selection, seen in every module: pick a bus in Topology, then follow
// the chip to Power flow, Case diff and the diagram.
// Linking is opt-in: until it is on, a pick stays in its own module.
const chip = page.getByTestId("selection-chip");
await page.getByRole("button", { name: "Topology", exact: true }).click();
await page.getByRole("tab", { name: /^Buses/ }).click();
const busRow = page.locator("table:visible tbody tr").nth(2);
const pickedBus = (await busRow.locator("td").first().innerText()).trim();
await busRow.click();
assert.equal(await chip.count(), 0, "unlinked selections stay local");
await page.getByRole("button", { name: "Link selection" }).click();
await busRow.click();
await chip.filter({ hasText: `Bus ${pickedBus}` }).waitFor();
await shot("selection-topology");
const selectedFirstCell = () => page.locator("tr[aria-selected=true]:visible td").first().innerText();
await chip.getByRole("button", { name: "Show in Power flow" }).click();
assert.equal((await selectedFirstCell()).trim(), pickedBus);
await shot("selection-power");
await chip.getByRole("button", { name: "Show in Case diff" }).click();
await page.locator("tr[aria-selected=true]:visible").first().waitFor();
assert.equal((await selectedFirstCell()).trim(), pickedBus);
await page.getByText("Original lines").waitFor();
await shot("selection-diff");
await chip.getByRole("button", { name: "Show in Diagram" }).click();
await page.locator(".react-flow__node-bus.selected").waitFor();
assert.equal(await page.locator(".react-flow__node-bus.selected").count(), 1);
await shot("selection-diagram");

// Back the other way: a symbol picked in the diagram is selected in Topology.
await page.locator(".react-flow__controls-fitview").click();
await page.waitForTimeout(400);
await page.locator(".react-flow__node-generator").first().click();
await chip.filter({ hasText: /Generator/ }).waitFor();
await chip.getByRole("button", { name: "Show in Topology" }).click();
await page.getByRole("tab", { name: /^Buses/, selected: true }).waitFor();
await page.locator("tr[aria-selected=true]:visible").first().waitFor();
await shot("selection-from-diagram");
await page.keyboard.press("Escape");
await chip.waitFor({ state: "detached" });
await page.getByRole("button", { name: "Diagram", exact: true }).click();

// Each base kV has its own colour, named in a legend; print style drops it.
const legend = page.getByTestId("voltage-legend");
assert.equal(await legend.innerText(), "138 kV\n69 kV", "legend lists the drawing's voltages");

// Print style, then undo after deleting everything.
await page.getByRole("button", { name: "Print style" }).click();
assert.equal(await legend.count(), 0, "no voltage legend in print style");
await shot("diagram-print");
await page.getByRole("button", { name: "Print style" }).click();
const nodeCount = await page.locator(".react-flow__node").count();
await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
await page.keyboard.press("Control+a");
await page.keyboard.press("Delete");
assert.equal(await page.locator(".react-flow__node").count(), 0);
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__node").count(), nodeCount);

// Labels: drag a generator's text clear of it (a leader line ties it back),
// undo and redo the move, drag a wire's flow text, then check that export and
// save keep the moved label where it was put.
const genLabel = page.locator(".react-flow__node-generator").first().locator("g.label-drag");
const labelAt = () => genLabel.getAttribute("transform");
const dragBy = async (loc, dx, dy) => {
  const b = await loc.boundingBox();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + dx, b.y + b.height / 2 + dy, { steps: 8 });
  await page.mouse.up();
};
const homeLabel = await labelAt();
await dragBy(genLabel, 60, 40);
await page.waitForFunction((t) => document.querySelector(".react-flow__node-generator g.label-drag").getAttribute("transform") !== t, homeLabel);
const movedLabel = await labelAt();
await page.locator(".react-flow__node-generator .label-leader").first().waitFor({ state: "attached" });
await page.keyboard.press("Control+z");
assert.equal(await labelAt(), homeLabel, "undo puts the label back in one step");
await page.keyboard.press("Control+Shift+z");
assert.equal(await labelAt(), movedLabel, "redo moves it again");
const flowText = page.locator("text.edge-label").first();
const flowX = await flowText.getAttribute("x");
await dragBy(flowText, -60, 0);
await page.waitForFunction((x) => document.querySelector("text.edge-label").getAttribute("x") !== x, flowX);
await shot("diagram-labels");
await queue("/tmp/labels.svg");
await page.getByRole("button", { name: "Export", exact: true }).click();
await page.locator("div.fixed button").filter({ hasText: /^SVG$/ }).click();
await page.locator("div.fixed button").filter({ hasText: /^Export/ }).click();
await page.waitForFunction(() => window.__files["/tmp/labels.svg"]);
const svgOut = await page.evaluate(() => window.__files["/tmp/labels.svg"]);
assert(svgOut.includes(`transform="${movedLabel}"`), "exported SVG keeps the moved label");
await queue("/tmp/labels.sld");
await page.getByRole("button", { name: "Save As", exact: true }).click();
await page.waitForFunction(() => window.__files["/tmp/labels.sld"]);
// Saved, so New asks nothing.
await page.getByRole("button", { name: "New", exact: true }).click();
await page.locator(".react-flow__node-generator").first().waitFor({ state: "detached" });
await queue("/tmp/labels.sld");
await page.getByRole("button", { name: "Open", exact: true }).click();
await page.locator(".react-flow__node-generator").first().waitFor();
assert.equal(await labelAt(), movedLabel, "a saved drawing reopens with the label where it was put");
await page.locator(".react-flow__node-generator").first().click({ position: { x: 20, y: 30 } });
await page.getByRole("button", { name: "Reset label position" }).click();
assert.equal(await labelAt(), homeLabel, "reset puts the label back");
await page.keyboard.press("Escape");

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

// Wire routing: a wire drawn past a symbol goes around it and its label.
await page.locator("[draggable=true]").filter({ hasText: /^Generator$/ }).dragTo(pane, { targetPosition: { x: 330, y: 300 } });
await load.hover();
const loadTerminal = await load.locator(".react-flow__handle").boundingBox();
await page.mouse.move(loadTerminal.x + loadTerminal.width / 2, loadTerminal.y + loadTerminal.height / 2);
await page.mouse.down();
await page.mouse.move(bar.x + 161, bar.y + bar.height + 4, { steps: 10 });
await page.mouse.up();
await page.locator(".react-flow__edge").first().waitFor();
/** Screen rectangles a wire's path passes through, and the generator's outline with its text. */
const wireVsSymbol = () =>
  page.evaluate(() => {
    const vp = document.querySelector(".react-flow__viewport");
    const m = new DOMMatrix(getComputedStyle(vp).transform);
    const origin = vp.parentElement.getBoundingClientRect();
    const d = document.querySelector(".react-flow__edge-path").getAttribute("d");
    const nums = d.match(/-?\d+(\.\d+)?/g).map(Number);
    const pts = [];
    for (let i = 0; i < nums.length; i += 2)
      pts.push({ x: origin.left + m.a * nums[i] + m.e, y: origin.top + m.d * nums[i + 1] + m.f });
    const gen = document.querySelector(".react-flow__node-generator");
    let box = gen.getBoundingClientRect();
    let [x0, y0, x1, y1] = [box.left, box.top, box.right, box.bottom];
    for (const el of gen.querySelectorAll("svg *")) {
      const r = el.getBoundingClientRect();
      if (!r.width && !r.height) continue;
      [x0, y0, x1, y1] = [Math.min(x0, r.left), Math.min(y0, r.top), Math.max(x1, r.right), Math.max(y1, r.bottom)];
    }
    let crosses = false;
    for (let i = 0; i < pts.length - 1; i++) {
      const [p, q] = [pts[i], pts[i + 1]];
      if (Math.max(p.x, q.x) > x0 && Math.min(p.x, q.x) < x1 && Math.max(p.y, q.y) > y0 && Math.min(p.y, q.y) < y1) crosses = true;
    }
    return { points: pts, crosses };
  });
await page.waitForFunction(() => document.querySelector(".react-flow__edge-path")?.getAttribute("d")?.split("L").length > 3);
assert.equal((await wireVsSymbol()).crosses, false, "routed wire avoids the generator");
await shot("diagram-routed");
// Moving the load reroutes its wire on release, still clear of the generator.
const loadBox = await load.boundingBox();
const path = await page.evaluate(() => document.querySelector(".react-flow__edge-path").getAttribute("d"));
await page.mouse.move(loadBox.x + loadBox.width / 2, loadBox.y + loadBox.height - 10);
await page.mouse.down();
await page.mouse.move(loadBox.x + loadBox.width / 2 - 80, loadBox.y + loadBox.height + 30, { steps: 8 });
await page.mouse.up();
await page.waitForFunction((before) => document.querySelector(".react-flow__edge-path").getAttribute("d") !== before, path);
await page.waitForTimeout(200);
assert.equal((await wireVsSymbol()).crosses, false, "rerouted wire avoids the moved generator");
// Drag a segment of the selected wire, then undo it.
{
  const { points } = await wireVsSymbol();
  // Click the middle of the longest segment to select the wire.
  let best = [points[0], points[1]];
  for (let i = 0; i < points.length - 1; i++) {
    const len = (p, q) => Math.abs(p.x - q.x) + Math.abs(p.y - q.y);
    if (len(points[i], points[i + 1]) > len(...best)) best = [points[i], points[i + 1]];
  }
  await page.mouse.click((best[0].x + best[1].x) / 2, (best[0].y + best[1].y) / 2);
}
const grip = page.locator(".segment-grips [data-segment] rect").nth(1);
await grip.waitFor();
const before = await page.evaluate(() => document.querySelector(".react-flow__edge-path").getAttribute("d"));
const g = await grip.boundingBox();
await page.mouse.move(g.x + g.width / 2, g.y + g.height / 2);
await page.mouse.down();
await page.mouse.move(g.x + g.width / 2 + 40, g.y + g.height / 2 + 40, { steps: 6 });
await page.mouse.up();
const after = await page.evaluate(() => document.querySelector(".react-flow__edge-path").getAttribute("d"));
assert.notEqual(after, before, "dragging a segment moves it");
await shot("diagram-segment-drag");
await page.getByText("Adjusted by hand").waitFor();
await page.keyboard.press("Control+z");
assert.equal(await page.evaluate(() => document.querySelector(".react-flow__edge-path").getAttribute("d")), before);

// On a fresh drawing, a generator dropped on the bar and a load dragged up to
// it sit on the bus and are wired to it with no visible line.
page.once("dialog", (d) => d.accept());
await page.getByRole("button", { name: "New", exact: true }).click();
await page.locator("[draggable=true]").filter({ hasText: /^Bus$/ }).dragTo(pane, { targetPosition: { x: 450, y: 200 } });
await page.locator("[draggable=true]").filter({ hasText: "Load (Static)" }).dragTo(pane, { targetPosition: { x: 330, y: 400 } });
const paneBox = await pane.boundingBox();
const barNow = await page.locator(".react-flow__node-bus").boundingBox();
await page.locator("[draggable=true]").filter({ hasText: /^Generator$/ }).dragTo(pane, {
  targetPosition: { x: barNow.x - paneBox.x + 40, y: barNow.y - paneBox.y + 20 },
});
// The wire has no length, so it counts as hidden: wait on the count.
await page.waitForFunction(() => document.querySelectorAll(".react-flow__edge").length === 1);
const gen = await page.locator(".react-flow__node-generator").boundingBox();
assert(Math.abs(gen.y - (barNow.y + barNow.height)) < 3, `generator hangs from the bar (${gen.y} vs ${barNow.y + barNow.height})`);
const loadAt = await load.boundingBox();
await page.mouse.move(loadAt.x + loadAt.width / 2, loadAt.y + 40);
await page.mouse.down();
await page.mouse.move(barNow.x + 160, barNow.y + barNow.height + 40, { steps: 10 });
await page.mouse.up();
await page.waitForFunction(() => document.querySelectorAll(".react-flow__edge").length === 2);
const onBus = await load.boundingBox();
assert(Math.abs(onBus.y - (barNow.y + barNow.height)) < 3, "dragged load snaps onto the bar");
await shot("diagram-on-bus");

// Rotation: R turns the selection a quarter. A turned bus runs vertically;
// symbols dropped beside it turn to face it and sit on it, and a wire released
// on the bar gets a tap on the side it comes from.
page.once("dialog", (d) => d.accept());
await page.getByRole("button", { name: "New", exact: true }).click();
await page.locator("[draggable=true]").filter({ hasText: /^Bus$/ }).dragTo(pane, { targetPosition: { x: 400, y: 150 } });
const vbus = page.locator(".react-flow__node-bus");
await vbus.click();
await page.keyboard.press("r");
await page.waitForFunction(() => {
  const b = document.querySelector(".react-flow__node-bus").getBoundingClientRect();
  return b.height > b.width;
});
const vb = await vbus.boundingBox();
const vx = vb.x - paneBox.x;
const vy = vb.y - paneBox.y;
await page.locator("[draggable=true]").filter({ hasText: "Load (Static)" }).dragTo(pane, { targetPosition: { x: vx + 20, y: vy + 60 } });
await page.locator("[draggable=true]").filter({ hasText: /^Generator$/ }).dragTo(pane, { targetPosition: { x: vx - 20, y: vy + 120 } });
await page.waitForFunction(() => document.querySelectorAll(".react-flow__edge").length === 2);
const sideLoad = await load.boundingBox();
assert(sideLoad.width > sideLoad.height, "load turned to face the vertical bus");
assert(Math.abs(sideLoad.x - (vb.x + vb.width)) < 3, `load sits on the right of the bar (${sideLoad.x} vs ${vb.x + vb.width})`);
const sideGen = await page.locator(".react-flow__node-generator").boundingBox();
assert(Math.abs(sideGen.x + sideGen.width - vb.x) < 3, "generator sits on the left of the bar");
await page.locator("[draggable=true]").filter({ hasText: "Circuit Breaker" }).dragTo(pane, { targetPosition: { x: vx + 200, y: vy + 160 } });
const cb = page.locator(".react-flow__node-circuitBreaker");
await cb.click();
await page.keyboard.press("r");
await page.waitForFunction(() => document.querySelector(".react-flow__node-circuitBreaker").getBoundingClientRect().width > 50);
await page.keyboard.press("Shift+R");
await page.waitForFunction(() => document.querySelector(".react-flow__node-circuitBreaker").getBoundingClientRect().width < 50);
await page.getByRole("button", { name: "Turn clockwise" }).click();
await page.getByTestId("rotation-angle").filter({ hasText: "90°" }).waitFor();
// The breaker's lower terminal now faces left, toward the bar.
const cbLeft = await page.locator('.react-flow__node-circuitBreaker [data-handleid="cb-bottom"]').boundingBox();
assert(cbLeft.x < (await cb.boundingBox()).x + 6, "turned terminal is on the left");
await page.mouse.move(cbLeft.x + cbLeft.width / 2, cbLeft.y + cbLeft.height / 2);
await page.mouse.down();
await page.mouse.move(vb.x + 3, vb.y + 170, { steps: 10 });
await page.mouse.up();
await page.waitForFunction(() => document.querySelectorAll(".react-flow__edge").length === 3);
const wire = await page.evaluate(() => [...document.querySelectorAll(".react-flow__edge-path")].map((p) => p.getAttribute("d")).find((d) => d && d.length > 20));
assert(wire, "wire from the breaker to the bar is drawn");
await page.mouse.click(paneBox.x + 5, paneBox.y + 5);
await shot("diagram-rotated");

// Inactive modules must not show the diagram through them.
await page.getByRole("button", { name: "Power flow", exact: true }).click();
const showing = await page.evaluate(
  () => [...document.querySelectorAll(".react-flow__node")].filter((n) => getComputedStyle(n).visibility === "visible").length
);
assert.equal(showing, 0, "diagram symbols visible on another module");

// Theme: the rail button cycles System, Light, Dark. Dark flips the app's
// colours, while the diagram in print style stays black on white.
const theme = page.getByRole("button", { name: /^Theme:/ });
for (let i = 0; i < 3 && (await theme.getAttribute("aria-label")) !== "Theme: Dark"; i++) await theme.click();
assert(await page.evaluate(() => document.documentElement.classList.contains("dark")), "dark theme applied");
const bodyBg = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--color-white"));
await shot("theme-dark");
for (let i = 0; i < 3 && (await theme.getAttribute("aria-label")) !== "Theme: Light"; i++) await theme.click();
assert(!(await page.evaluate(() => document.documentElement.classList.contains("dark"))), "light theme applied");
assert.notEqual(bodyBg, await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--color-white")), "theme changes colours");

// DC lines: one hop around a DC rectifier bus draws each DC line as a
// converter pair down to its inverter bus, and the legend names it.
if (STANNUM_RAW_DC) {
  await queue(STANNUM_RAW_DC);
  await page.getByRole("button", { name: "Open another…" }).click();
  await page.getByRole("button", { name: "Diagram", exact: true }).click();
  await page.getByRole("button", { name: "From case" }).click();
  await page.getByLabel(/Around a bus/).check();
  await page.getByRole("combobox", { name: "Bus number" }).fill("301");
  await page.getByLabel("Hops out").selectOption("1");
  page.once("dialog", (d) => d.accept()); // replace the edited drawing
  await page.getByRole("button", { name: "Generate", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "detached" });
  await page.locator(".react-flow__node-dcLink").first().waitFor();
  assert.equal(await page.locator(".react-flow__node-dcLink").count(), 2, "both DC lines from bus 301 drawn");
  assert.match(await page.getByTestId("voltage-legend").innerText(), /DC line/, "legend names DC lines");
  await shot("diagram-dc");

  // VSC and multi-terminal DC use the same symbol with their own caption.
  const around = async (bus) => {
    await page.getByRole("button", { name: "From case" }).click();
    await page.getByLabel(/Around a bus/).check();
    await page.getByRole("combobox", { name: "Bus number" }).fill(String(bus));
    await page.getByLabel("Hops out").selectOption("1");
    page.once("dialog", (d) => d.accept());
    await page.getByRole("button", { name: "Generate", exact: true }).click();
    await page.getByRole("dialog").waitFor({ state: "detached" });
    await page.locator(".react-flow__node-dcLink").first().waitFor();
    await page.locator(".react-flow__controls-fitview").click();
    await page.waitForTimeout(400);
  };
  await around(3005);
  const vsc = page.locator(".react-flow__node-dcLink", { hasText: "VSC DC" });
  assert.equal(await vsc.count(), 1, "VSC line from bus 3005 drawn");
  assert.match(await vsc.innerText(), /VSC VDCLINE1/);
  await shot("diagram-vsc");
  // Bus 212 holds the DC voltage of the multi-terminal system, so all three
  // scheduled converters link to it.
  await around(212);
  assert.equal(
    await page.locator(".react-flow__node-dcLink", { hasText: "Multi-terminal DC" }).count(),
    3,
    "three multi-terminal converters linked to bus 212",
  );
  await shot("diagram-mtdc");

  // Power flow lists every DC line; with Q limits on, the case converges and
  // the line-commutated converters absorb reactive power. Two-terminal lines
  // use the detailed converter model: the rectifier holds the order.
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByLabel("Enforce generator Q limits").check();
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  await page.getByText("Converged").first().waitFor();
  await page.getByRole("tab", { name: /DC lines/ }).click();
  await page.getByText("P received (MW)").waitFor();
  const dcRows = page.locator("tr:visible", { hasText: /Two-terminal|VSC|Multi-terminal/ });
  assert.equal(await dcRows.count(), 7, "seven DC rows listed");
  const first = await dcRows.first().innerText();
  assert.match(first, /Two-terminal[\s\S]*301[\s\S]*3021[\s\S]*-\d/, "rectifier absorbs Q");
  assert.match(first, /Rectifier/, "detailed model reports the controlling converter");
  const vscRow = await dcRows.nth(2).innerText();
  assert.match(vscRow, /VSC[\s\S]*At order/, "detailed VSC model reports its order state");
  await shot("power-dc-lines");
  await page.getByLabel("Enforce generator Q limits").uncheck();
}

// A DC line picked on the diagram is found in Case diff, and back again.
// TWO-TERMINAL DC records are keyed by name, so this checks that the diff
// names them by their buses the way the diagram does.
if (STANNUM_DC_A && STANNUM_DC_B) {
  const chip = page.getByTestId("selection-chip");
  await queue(STANNUM_DC_A);
  await page.getByRole("button", { name: "Open another…" }).click();
  await page.getByRole("button", { name: "Case diff", exact: true }).click();
  await page.getByText(/Use the open case/).click();
  await queue(STANNUM_DC_B);
  await page.getByRole("button", { name: "Browse…" }).nth(1).click();
  await page.getByRole("button", { name: /Compare files/ }).click();
  await page.getByText(/differing records/).first().waitFor();
  await page.getByRole("button", { name: /^TWO-TERMINAL DC/ }).waitFor();

  await page.getByRole("button", { name: "Diagram", exact: true }).click();
  await page.getByRole("button", { name: "From case" }).click();
  await page.getByLabel(/Whole case/).check();
  page.once("dialog", (d) => d.accept()); // replace the edited drawing
  await page.getByRole("button", { name: "Generate", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "detached" });
  await page.waitForTimeout(400); // fit-to-screen animation
  const dc = page.locator(".react-flow__node-dcLink");
  assert.equal(await dc.count(), 1, "one DC line drawn");
  await dc.click();
  await chip.filter({ hasText: /DC line East link/ }).waitFor();
  await shot("dc-diff-picked");
  await chip.getByRole("button", { name: "Show in Case diff" }).click();
  const picked = page.locator("tr[aria-selected=true]:visible").first();
  await picked.waitFor();
  assert.match(await picked.innerText(), /East link/);
  await page.getByText(/is not in this comparison/).waitFor({ state: "detached" });
  const setvl = page.locator("tr:visible", { hasText: "SETVL" });
  assert.match(await setvl.innerText(), /40\.00[\s\S]*55\.00/, "SETVL change shown");
  await shot("dc-diff-found");

  // Picking the row in Case diff selects the symbol on the diagram.
  await page.keyboard.press("Escape");
  await chip.waitFor({ state: "detached" });
  await page.locator("section:has-text('Records') table tbody tr", { hasText: "East link" }).click();
  await chip.filter({ hasText: /DC line East link/ }).waitFor();
  await chip.getByRole("button", { name: "Show in Diagram" }).click();
  await page.locator(".react-flow__node-dcLink.selected").waitFor();
  await shot("dc-diff-back");
}

assert.deepEqual(errors, [], `page errors: ${errors.join("; ")}`);
await browser.close();
console.log("smoke test passed");
