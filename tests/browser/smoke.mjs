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

const { STANNUM_MATPOWER, STANNUM_RAW, STANNUM_RAW_B, STANNUM_RAW_DC, STANNUM_DC_A, STANNUM_DC_B, STANNUM_VIOLATIONS, STANNUM_VSC_CASE, STANNUM_VSC_GRIDS, STANNUM_SHOTS } = process.env;
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
await page.getByTestId("top-branches").getByText("%").first().waitFor();
await shot("power-overview");
await page.getByRole("tab", { name: /Branches/ }).click();
await page.getByText("Loading (%)").waitFor();
await page.getByTestId("loading-bar").first().waitFor();
await shot("power-branches");
await page.getByRole("button", { name: "Calculate sensitivities" }).click();
await page.getByText("How to read these results").waitFor();
await page.getByText(/Response \(/).waitFor();
await shot("power-sensitivities");

// The log records each step: the app's own lines and the solver's, pass by
// pass and iteration by iteration. Ctrl+` opens and closes it.
const log = page.getByRole("log", { name: "Log lines" });
await page.keyboard.press("Control+Backquote");
await log.getByText(/Solved power flow \(/).last().waitFor();
await log.getByText(/Finished AC power flow: converged/).last().waitFor();
await log.getByText(/Newton iteration 1: largest P mismatch .* MW at bus/).last().waitFor();
await log.getByText(/Totals: generation/).last().waitFor();
await log.getByText(/Sensitivities: \d+ injection columns/).last().waitFor();
assert.equal(await log.getByText(/assembling active network/).count(), 0, "detail lines are hidden by default");
await page.getByTestId("log-dock").getByLabel("Details").check();
await log.getByText(/assembling active network/).first().waitFor();
await shot("log");
await page.getByTestId("log-dock").getByLabel("Details").uncheck();
await page.getByRole("button", { name: "Close log" }).click();
await log.waitFor({ state: "detached" });
await page.getByTestId("log-status").filter({ hasText: /Sensitivities|Calculated sensitivities/ }).waitFor();

// A study: save the solve as filed, take line 4-5 out, and the case
// re-solves without it.
await page.getByRole("tab", { name: /Study/ }).click();
await page.getByRole("button", { name: "Save this solve" }).click();
await page.getByTestId("saved-solves").getByRole("textbox", { name: "Solve name" }).first().waitFor();
assert.equal(await page.getByTestId("saved-solves").getByRole("textbox").first().inputValue(), "Case as filed");
await page.getByRole("tab", { name: /Branches/ }).click();
const line45 = page.locator("table:visible tbody tr:not([data-spacer])").nth(1);
assert.equal((await line45.locator("td").nth(1).innerText()).trim(), "4");
await line45.click();
await page.getByTestId("table-selection").getByRole("button", { name: "Take out" }).click();
await page.getByTestId("study-badge").filter({ hasText: "1 edit" }).waitFor();
await line45.locator("td", { hasText: /^Offline$/ }).waitFor();
assert.equal((await line45.locator("td").nth(7).innerText()).trim(), "0.00", "no flow on the outaged line");
await page.getByTestId("table-selection").getByRole("button", { name: "Put back" }).waitFor();
// Redispatch a generator too.
await page.getByRole("tab", { name: /Generators/ }).click();
await page.locator("table:visible tbody tr:not([data-spacer])").nth(1).click();
await page.getByTestId("table-selection").getByRole("button", { name: "Set output" }).click();
await page.getByRole("dialog", { name: "Set output" }).getByLabel("MW").fill("120");
await page.getByRole("button", { name: "Apply to study" }).click();
await page.getByTestId("study-badge").filter({ hasText: "2 edits" }).waitFor();
await page.locator("table:visible tbody tr:not([data-spacer])").nth(1).locator("td", { hasText: /^120\.00$/ }).waitFor();
await page.getByTestId("study-badge").click();
const edits = page.getByTestId("study-edits");
await edits.getByText("Line 4–5 circuit 1 out of service").waitFor();
await edits.getByText(/Generator 1 at bus 2 set to 120\.0 MW \(was 163\.0 MW\)/).waitFor();
await shot("study-edits");
// Compare the saved solve with now: line 4-5 taken out, its flow moved elsewhere.
const comparison = page.getByTestId("solve-comparison");
await comparison.waitFor();
await page.getByTestId("compare-branches").locator("td", { hasText: "Taken out" }).waitFor();
await page.getByTestId("solve-comparison").scrollIntoViewIfNeeded();
await shot("study-compare");
await page.getByRole("tab", { name: "Overview" }).click();
await shot("study-overview");
await page.getByRole("tab", { name: /Study/ }).click();
await page.getByRole("button", { name: "Clear all" }).click();
await page.getByTestId("study-badge").waitFor({ state: "detached" });

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
// Branch tables name both ends when the case names its buses.
await page.getByRole("tab", { name: /Branches/ }).click();
await page.getByRole("columnheader", { name: "From name" }).waitFor();
await page.locator("table:visible tbody tr").first().locator("td").nth(2).filter({ hasText: /\S/ }).waitFor();
await shot("tables-names");
await page.getByRole("tab", { name: "Overview" }).click();

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
await page.locator("svg text", { hasText: /\d\.\d{3} pu/ }).first().waitFor();
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
await page.getByText(/^Around\b/).waitFor();
assert.equal(await page.locator(".react-flow__node-bus").count(), 3, "bus 4 and its neighbours 2 and 5");
const studied = page.locator(".react-flow__node-bus").filter({ has: page.locator("title", { hasText: "Studied bus" }) });
assert.equal(await studied.count(), 1, "studied bus highlighted");
// Branches leaving one bus share a marker that counts them.
const markers = page.locator(".react-flow__node-offPage");
assert.equal(await markers.count(), 1, "one marker for the branches toward buses 1 and 3");
assert.equal((await markers.getByTestId("off-page-count").textContent()).trim(), "2");
await markers.filter({ hasText: "To 2 buses" }).waitFor();
await page.waitForTimeout(400);
const studiedAt = await page.evaluate(() => document.querySelector('[data-id="b4"]').style.transform);
await markers.first().dblclick();
await page.waitForFunction(() => document.querySelectorAll(".react-flow__node-bus").length === 5);
assert.equal(await page.evaluate(() => document.querySelector('[data-id="b4"]').style.transform), studiedAt, "expand moved bus 4");
assert.equal(await markers.count(), 0, "both branches drawn, so the marker goes");
await page.waitForTimeout(500); // wires go in a frame after the taps
await shot("diagram-local");
await page.keyboard.press("Escape");
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__node-bus").count(), 3, "undo removes the expansion");
// One bus brought in from the bundle's menu leaves a plain marker for the other.
await markers.first().click({ button: "right" });
const bringIn = page.getByRole("menuitem", { name: /^Bring in (?!all)/ });
assert.equal(await bringIn.count(), 2, "each far bus offered");
await bringIn.first().click();
await page.waitForFunction(() => document.querySelectorAll(".react-flow__node-bus").length === 4);
assert.equal(await markers.getByTestId("off-page-count").count(), 0, "the bundle is down to a single branch");
await page.keyboard.press("Escape");
await page.keyboard.press("Control+z");
assert.equal(await page.locator(".react-flow__node-bus").count(), 3);

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
/** Whether the wire still crosses the generator once routing settles (up to 3 s). */
const settledCrossing = async () => {
  const until = Date.now() + 3000;
  let { crosses } = await wireVsSymbol();
  while (crosses && Date.now() < until) {
    await page.waitForTimeout(100);
    ({ crosses } = await wireVsSymbol());
  }
  return crosses;
};
await page.waitForFunction(() => document.querySelector(".react-flow__edge-path")?.getAttribute("d")?.split("L").length > 3);
// The route can be redrawn once more as the new symbols finish measuring,
// so give it a moment to settle before checking.
assert.equal(await settledCrossing(), false, "routed wire avoids the generator");
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
assert.equal(await settledCrossing(), false, "rerouted wire avoids the moved generator");
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
  const mtRow = await dcRows.nth(4).innerText();
  assert.match(mtRow, /Multi-terminal[\s\S]*At order/, "detailed multi-terminal model reports its converter state");
  await shot("power-dc-lines");
  // The DC network tab lists the multi-terminal system's buses and links.
  await page.getByRole("tab", { name: /DC network/ }).click();
  const network = page.getByTestId("dc-network");
  await network.getByText("Holds DC voltage").waitFor();
  assert.match(await network.innerText(), /DC5[\s\S]*517\.\d/, "star bus DC5 solved near 517 kV");
  await shot("power-dc-network");
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

// VSC DC grids: attach a Stannum DC grid file to a case, solve, and read the
// converters, DC buses and DC lines of each grid.
if (STANNUM_VSC_CASE && STANNUM_VSC_GRIDS) {
  await queue(STANNUM_VSC_CASE);
  await page.getByRole("button", { name: /Open (case|another)…/ }).first().click();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByTestId("open-vsc-grids").click();
  await queue(STANNUM_VSC_GRIDS);
  await page.getByRole("button", { name: "Attach DC grid file…" }).click();
  await page.getByTestId("dc-grid-file").getByText("two_area.dcgrid.json").waitFor();
  await page.getByText("Solve to include these DC grids.").waitFor();
  await shot("vsc-grids-attached");
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  const tab = page.getByTestId("vsc-grids");
  await tab.getByText("AC/DC iterations").waitFor();
  assert.equal(await tab.getByTestId("vsc-grid").count(), 2, "both DC grids reported");
  await tab.getByRole("cell", { name: "Offshore", exact: true }).first().waitFor();
  await tab.getByRole("cell", { name: "DC slack" }).waitFor();
  await tab.getByRole("cell", { name: "Droop" }).first().waitFor();
  await shot("vsc-grids");
  await tab.getByTestId("vsc-grid").nth(1).scrollIntoViewIfNeeded();
  await shot("vsc-grids-droop");
  await page.getByRole("tab", { name: "Overview" }).click();
  await page.getByText("Converged").first().waitFor();
}

// Findings lead somewhere: on a case with limit violations, a check opens the
// violations worst first, and "Draw around" opens the diagram on the element.
// Selections are unlinked here, so this works without linking.
if (STANNUM_VIOLATIONS) {
  await queue(STANNUM_VIOLATIONS);
  await page.getByRole("button", { name: /Open (case|another)…/ }).first().click();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  const linkedToggle = page.getByRole("button", { name: "Linked", exact: true });
  if (await linkedToggle.count()) await linkedToggle.click();
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  await page.getByText("Operating limits").waitFor();
  const voltage = page.getByTestId("check-buses");
  const worst = (await voltage.getByRole("button", { name: /^Bus \d+$/ }).innerText()).trim();
  const violations = Number((await voltage.locator("td").nth(1).innerText()).replace(/,/g, ""));
  assert(violations > 0, "the case has voltage violations");
  await shot("findings-overview");

  // The overview pictures every bus voltage against its limits, and the
  // branches carrying the most (case300 has no ratings, so the largest flows).
  const profile = page.getByTestId("voltage-profile");
  await profile.scrollIntoViewIfNeeded();
  assert.ok((await profile.locator("circle.fill-red-600").count()) === violations, "every violation drawn red");
  const top = page.getByTestId("top-branches");
  await top.getByText(/largest flows/).waitFor();
  const svgBox = await profile.locator("svg").boundingBox();
  await page.mouse.move(svgBox.x + 60, svgBox.y + svgBox.height / 2);
  await page.getByTestId("voltage-profile-tip").waitFor();
  await shot("overview-charts");
  await page.mouse.move(0, 0);

  await voltage.locator("td").nth(1).getByRole("button").click();
  const only = page.getByLabel(/Voltage violations only/);
  assert.equal(await only.isChecked(), true, "only violations listed");
  await page.getByText(new RegExp(`^${violations} rows?$`)).waitFor();
  const first = page.locator("table tbody tr:visible").first();
  assert.equal((await first.locator("td").first().innerText()).trim(), worst.replace("Bus ", ""), "worst first");
  await first.click();
  await page.getByTestId("table-selection").filter({ hasText: worst }).waitFor();
  await shot("findings-violations");

  await page.getByTestId("table-selection").getByRole("button", { name: /Draw around/ }).click();
  await page.locator(".react-flow__node-bus.selected").first().waitFor();
  await page.getByText(/^Around\b/).first().waitFor();
  const studied = page.locator(".react-flow__node-bus").filter({ has: page.locator("title", { hasText: "Studied bus" }) });
  assert.equal(await studied.count(), 1, "drawn around the worst bus");
  await page.waitForTimeout(600); // centred after the fit
  await shot("findings-drawn");

  // A drawing from the case keeps the components palette folded, and the
  // inspector only appears with a selection, so the drawing gets the room.
  assert.equal(await page.getByTestId("components-palette").count(), 0, "palette folded on a generated drawing");
  await page.getByTestId("inspector").waitFor();
  await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
  await page.getByTestId("inspector").waitFor({ state: "detached" });
  await shot("diagram-roomy");

  // Zoomed out, results turn into marks: heavier wires for loading, bigger
  // arrows, and a band on buses near or past their limits. Numbers return on
  // hover and when zoomed back in.
  assert.ok((await page.getByTestId("voltage-band").count()) > 0, "the worst bus is banded");
  const flowText = page.locator(".edge-label", { hasText: /\d MW/ });
  assert.ok((await flowText.count()) > 0, "flows written out when close");
  await page.locator(".react-flow__controls-fitview").click();
  await page.waitForTimeout(400);
  const pane = page.locator(".react-flow__pane");
  const box = await pane.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let i = 0; i < 12 && (await page.getByTestId("result-mark").count()) === 0; i++) {
    await page.mouse.wheel(0, 300);
    await page.waitForTimeout(150);
  }
  assert.ok((await page.getByTestId("result-mark").count()) > 0, "flows drawn as marks when far");
  assert.equal(await flowText.count(), 0, "no flow text when far");
  assert.ok((await page.getByTestId("voltage-band").count()) > 0, "band still shown when far");
  await shot("diagram-marks");
  // The same marks on the dark canvas, switched from the palette.
  const isDark = () => page.evaluate(() => document.documentElement.classList.contains("dark"));
  const wasDark = await isDark();
  const cycleTheme = async () => {
    await page.keyboard.press("Control+k");
    await page.getByRole("combobox", { name: "Search" }).fill("theme");
    await page.getByRole("option", { name: /theme/ }).first().waitFor();
    await page.keyboard.press("Enter");
  };
  for (let i = 0; i < 3 && !(await isDark()); i++) await cycleTheme();
  await page.waitForTimeout(200);
  await shot("diagram-marks-dark");
  for (let i = 0; i < 3 && (await isDark()) !== wasDark; i++) await cycleTheme();
  await page.locator(".react-flow__controls-fitview").click();
  await page.getByRole("button", { name: "Show components" }).click();
  await page.getByTestId("components-palette").waitFor();
  await page.getByRole("button", { name: "Hide components" }).click();
  await page.getByTestId("components-palette").waitFor({ state: "detached" });

  // The overview's location link draws straight from the check.
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByRole("tab", { name: "Overview" }).click();
  const gen = page.getByTestId("check-generators");
  if (Number((await gen.locator("td").nth(1).innerText()).replace(/,/g, "")) > 0) {
    await gen.getByRole("button", { name: /^Draw a one-line around/ }).click();
    await page.getByTestId("draw-request").waitFor({ state: "detached" });
    await page.locator(".react-flow__node.selected").first().waitFor();
  }

  // Ctrl+K finds a bus by number and shows it everywhere, linked or not.
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Search and commands" });
  await palette.waitFor();
  // Bus 7 is within limits, so the violations-only filter must step aside for it.
  await palette.getByRole("combobox").fill("7");
  await palette.getByRole("option").first().filter({ hasText: /^Bus 7(?!\d)/ }).waitFor();
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByRole("tab", { name: /Buses/ }).click();
  const row7 = page.locator("tr[aria-selected=true]:visible").first();
  await row7.waitFor();
  assert.equal((await row7.locator("td").first().innerText()).trim(), "7", "bus 7 shown though within limits");
  assert.equal(await page.getByLabel(/Voltage violations only/).isChecked(), false);
  await page.getByRole("button", { name: "Diagram", exact: true }).click();
  await page.keyboard.press("Control+k");
  await palette.getByRole("combobox").fill("170");
  const firstHit = palette.getByRole("option").first();
  await firstHit.filter({ hasText: /^Bus 170/ }).waitFor();
  await shot("palette-bus");
  await page.keyboard.press("Enter");
  await palette.waitFor({ state: "detached" });
  await page.getByTestId("selection-chip").filter({ hasText: "Bus 170" }).waitFor();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  const row170 = page.locator("tr[aria-selected=true]:visible").first();
  await row170.waitFor();
  assert.equal((await row170.locator("td").first().innerText()).trim(), "170", "bus 170 shown in Power flow");
  await shot("palette-shown");

  // Commands run from the palette too, and Shift+Enter draws around a result.
  await page.keyboard.press("Control+k");
  await palette.getByRole("combobox").fill("solve");
  await palette.getByRole("option", { name: /Solve power flow/ }).waitFor();
  await page.keyboard.press("Enter");
  await page.getByRole("tab", { name: "Overview", selected: true }).waitFor();
  await page.locator("text=Converged >> visible=true").first().waitFor();
  await page.keyboard.press("Control+k");
  await palette.getByRole("combobox").fill("170");
  await palette.getByRole("option").first().filter({ hasText: /^Bus 170/ }).waitFor();
  await page.keyboard.press("Shift+Enter");
  await page.getByText(/^Around\b/).filter({ hasText: "170" }).first().waitFor();
  await page.keyboard.press("Escape");

  // Side by side: the drawing on the left, Power flow on the right, with
  // selection linked between them while split.
  await page.getByRole("button", { name: "Side by side" }).click();
  const right = page.locator("[data-pane=right]");
  await right.getByRole("combobox", { name: "Right pane" }).waitFor();
  assert.equal(await right.getByRole("combobox", { name: "Right pane" }).inputValue(), "power");
  await page.getByRole("button", { name: "Linked", exact: true }).waitFor();
  await right.getByRole("tab", { name: /Buses/ }).click();
  const violationsBox = right.getByLabel(/Voltage violations only/);
  if (await violationsBox.isChecked()) await violationsBox.uncheck();
  await right.getByPlaceholder("Search buses…").fill("170");
  await right.locator("tbody tr:not([data-spacer])", { has: page.locator("td:first-child", { hasText: /^170$/ }) }).click();
  await page.locator("[data-pane=left] .react-flow__node-bus.selected").first().waitFor();
  await page.waitForTimeout(400);
  await shot("split-view");
  await right.getByRole("button", { name: "Swap sides" }).click();
  assert.equal(await page.locator("[data-pane=left]").getByRole("combobox", { name: "Left pane" }).inputValue(), "power");
  await page.locator("[data-pane=right]").getByRole("button", { name: "Close split view" }).click();
  await page.locator("[data-pane]").first().waitFor({ state: "detached" });
  await page.getByRole("button", { name: "Link selection" }).waitFor();

  // Drawn around a branch, a local drawing grows from both its ends (neither
  // is the studied bus) and the branch itself is marked.
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByRole("tab", { name: /Branches/ }).click();
  const branchRow = page.locator("table:visible tbody tr:not([data-spacer])").first();
  await branchRow.click();
  await page.getByTestId("table-selection").getByRole("button", { name: /Draw around/ }).click();
  await page.getByText(/^Around\b/).filter({ hasText: /Line|Transformer/ }).first().waitFor();
  await page.getByTestId("draw-request").waitFor({ state: "detached" });
  await page.getByTestId("focus-halo").first().waitFor();
  assert.equal(await studied.count(), 0, "a branch has no studied bus");
  await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
  await page.locator(".react-flow__controls-fitview").click();
  await page.waitForTimeout(400);
  await shot("around-branch");

  // An empty drawing offers to draw around the worst violation.
  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "New", exact: true }).click();
  const prompt = page.getByTestId("draw-prompt");
  await prompt.waitFor();
  await prompt.getByRole("button", { name: new RegExp(`worst violation: ${worst}(?!\\d)`) }).waitFor();
  await shot("draw-prompt");
  await prompt.getByRole("button", { name: /worst violation/ }).click();
  await prompt.waitFor({ state: "detached" });
  await page.getByText(/^Around\b/).filter({ hasText: /\S/ }).first().waitFor();
  await studied.first().waitFor();
  assert.equal(await studied.count(), 1, "drawn around the worst bus");

  // Tables draw only the rows in view, carry bus names, and show loading and
  // limits as bars and marks.
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByRole("tab", { name: /Branches/ }).click();
  const drawnRows = page.locator("table:visible tbody tr:not([data-spacer])");
  const overloadsOnly = page.getByLabel(/Overloads only/);
  if (await overloadsOnly.isChecked()) await overloadsOnly.uncheck();
  const total = Number((await page.locator("text=/^[\\d,]+ rows?$/ >> visible=true").first().innerText()).replace(/[^\d]/g, ""));
  assert.ok(total > 300, "case300 has more than 300 branches");
  assert.ok((await drawnRows.count()) < total, "only rows in view are drawn");
  const scroller = page.locator("table:visible").locator("xpath=..");
  await scroller.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await page.locator("table:visible tbody tr:not([data-spacer]) td:first-child", { hasText: new RegExp(`^${total}$`) }).waitFor();
  await scroller.evaluate((el) => (el.scrollTop = 0));
  await page.getByRole("columnheader", { name: /Loading/ }).click();
  await page.getByRole("columnheader", { name: /Loading/ }).click();
  await shot("tables-branches");
  await page.getByRole("tab", { name: /Buses/ }).click();
  await page.getByTestId("range-mark").first().waitFor();
  await page.getByRole("columnheader", { name: "Vm (p.u.)" }).click();
  await shot("tables-buses");

  // Filter chips narrow the bus, generator and branch tables together.
  const violationsOnly = page.getByLabel(/Voltage violations only/);
  if (await violationsOnly.isChecked()) await violationsOnly.uncheck();
  const chips = page.getByTestId("filter-chips");
  await chips.getByRole("button", { name: "Add filter" }).click();
  const editor = page.getByRole("dialog", { name: "Edit filter" });
  await editor.getByLabel("Filter field").selectOption("base_kv");
  await editor.getByLabel("Filter value").selectOption("230");
  await editor.getByRole("button", { name: "Done" }).click();
  await chips.getByTestId("filter-chip").filter({ hasText: "Base kV is 230" }).waitFor();
  const kept = Number((await chips.getByText(/ of 300 buses$/).innerText()).split(" ")[0]);
  assert.ok(kept > 0 && kept < 300, "some buses kept");
  for (const kv of await page.locator("table:visible tbody tr:not([data-spacer]) td:nth-child(4)").allInnerTexts())
    assert.equal(kv.trim(), "230.0", "only 230 kV buses listed");
  await page.getByRole("tab", { name: /Branches/ }).click();
  await chips.getByText(/touching matching buses$/).waitFor();
  await shot("filter-chips");
  // The same filters narrow the overview's charts; its checks stay whole-case.
  await page.getByRole("tab", { name: "Overview" }).click();
  // The table count can include the selected bus; the charts show only matches.
  const charted = Number((await chips.getByText(/^Charts show \d+ of 300 buses/).innerText()).split(" ")[2]);
  assert.ok(charted > 0 && charted <= kept, "charts narrowed to the matching buses");
  await page.getByText(`${charted} of 300 buses, lowest to highest`).waitFor();
  await page.getByText("Touching matching buses, against rating A").waitFor();
  await page.getByTestId("voltage-profile").scrollIntoViewIfNeeded();
  await shot("overview-filtered");
  await page.getByRole("tab", { name: /Branches/ }).click();
  await chips.getByRole("button", { name: /^Remove filter/ }).click();
  await chips.getByText("Applies to buses, generators and branches").waitFor();

  // MATPOWER has no RAW records, so Topology is built from the power flow model.
  await page.getByRole("button", { name: "Topology", exact: true }).click();
  await page.getByText("Built from the power flow model.").waitFor();
  await page.locator("canvas:visible").first().waitFor();
  await page.getByText("Articulation buses").first().waitFor();
  await shot("topology-matpower");
}

assert.deepEqual(errors, [], `page errors: ${errors.join("; ")}`);
await browser.close();
console.log("smoke test passed");
