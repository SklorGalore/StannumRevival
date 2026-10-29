// Browser smoke test: drives the real UI against the real Rust core.
//
//   cargo run --release -p stannum-core --example dev_bridge &   # :1421
//   npx vite --port 1420 &
//   (for the crash test: build the bridge with --features fault-injection, run
//   it with STANNUM_FAULT_COMMAND=fault_test, and set STANNUM_CRASH_TEST=1)
//   NODE_PATH=$(npm root -g) STANNUM_MATPOWER=case9.m STANNUM_RAW=a.raw STANNUM_RAW_B=b.raw node tests/browser/smoke.mjs
//
// Tauri's IPC is replaced by a shim that forwards commands to the bridge and
// answers file dialogs from a queue. Screenshots go to $STANNUM_SHOTS if set.
import { createRequire } from "node:module";
// Resolved through require() so a globally installed Playwright works via NODE_PATH.
const { chromium } = createRequire(import.meta.url)("playwright");
import assert from "node:assert/strict";

const { STANNUM_CRASH_TEST } = process.env;
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
      // The crash test sends the next call of this command to the bridge's
      // fault_test command, which panics (STANNUM_FAULT_COMMAND=fault_test).
      if (cmd === "stannum_command" && args?.command === window.__faultNext) {
        window.__faultNext = null;
        args = { ...args, command: "fault_test" };
      }
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
// Questions such as "discard unsaved changes?" are answered yes, and kept.
const dialogs = [];
page.on("dialog", (d) => {
  dialogs.push(d.message());
  void d.accept();
});
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
await log.getByText(/(Fixed-slope|Fast decoupled|Newton) iteration 1: largest P mismatch .* MW at bus/).last().waitFor();
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

// N-1 contingencies: every branch and generator out in turn, ranked, and
// the worst opened beside the table.
await page.getByRole("tab", { name: /Contingencies/ }).click();
await page.getByTestId("run-contingencies").click();
await page.getByTestId("contingency-summary").getByText("Outages").waitFor();
await page.getByTestId("contingency-detail").getByText(/Overloads above/).waitFor();
await page.getByTestId("contingency-detail").getByText(/AC solve, converged|AC solve did not converge|Linear screen only/).waitFor();
await shot("power-contingencies");

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
// The element inspector opens beside the table: results, limits and the
// file's data for the line, with the study actions.
const inspector = page.getByTestId("power-inspector");
await inspector.getByTestId("inspector-title").filter({ hasText: "Line 4–5 circuit 1" }).waitFor();
await inspector.getByTestId("inspector-headline").filter({ hasText: /% of rate A/ }).waitFor();
await inspector.getByTestId("inspector-limits").getByText("Rate A").waitFor();
await inspector.getByTestId("inspector-data").getByText("Reactance").first().waitFor();
await shot("inspector-power-line");
// Trip and compare: take the line out of the latest solve, solve again,
// and draw what changes around it on the diagram.
await inspector.getByTestId("trip-and-compare").click();
await page.getByText(/^Around\b/).first().waitFor();
await page.getByTestId("layer-legend").filter({ hasText: "Out: Line 4–5 circuit 1" }).waitFor();
const tripped = page.locator("[data-testid=trip-outcome]:visible");
await tripped.getByText("If it trips").waitFor();
await tripped.getByText(/Solved in AC|did not converge/).waitFor();
await shot("trip-and-compare");
// Every view shows the tripped case now: the case bar's result chip names it.
await page.locator('[data-testid=result-chip][data-result="trip line:4:5:1"]').filter({ hasText: "Tripped" }).waitFor();
// Colour the drawing by the latest solve's branch loading, then by voltage.
await page.getByRole("button", { name: "Layers", exact: true }).click();
const resultLayer = page.getByTestId("layers-panel").locator('[data-layer="results"]');
await resultLayer.getByRole("checkbox").check();
await page.getByTestId("layer-legend").filter({ hasText: "Colour by result" }).filter({ hasText: /under 50%|50–80%/ }).waitFor();
await resultLayer.getByLabel("Result to colour by").selectOption("voltage");
await page.getByTestId("layer-legend").filter({ hasText: /p\.u\.|0\.98–1\.02/ }).waitFor();
await shot("diagram-colour-by-result");
await resultLayer.getByRole("checkbox").uncheck();
await tripped.getByRole("button", { name: "Clear" }).click();
await tripped.waitFor({ state: "detached" });
await page.locator('[data-testid=result-chip][data-result="base"]').filter({ hasText: "Base case" }).waitFor();
await page.getByRole("button", { name: "Hide layers" }).click();
await page.getByRole("button", { name: "Power flow", exact: true }).click();
await inspector.getByTestId("inspector-title").filter({ hasText: "Line 4–5 circuit 1" }).waitFor();
await inspector.getByRole("button", { name: "Take out" }).click();
await page.getByTestId("study-badge").filter({ hasText: "1 edit" }).waitFor();
await line45.locator("td", { hasText: /^Offline$/ }).waitFor();
assert.equal((await line45.locator("td").nth(7).innerText()).trim(), "0.00", "no flow on the outaged line");
await inspector.getByRole("button", { name: "Put back" }).waitFor();
await inspector.getByTestId("inspector-status").filter({ hasText: "Out in this study" }).waitFor();
await inspector.getByTestId("inspector-edits").getByText("Line 4–5 circuit 1 out of service").waitFor();
await shot("inspector-power-outage");
// A terminal bus in the inspector opens that bus, in its table.
await inspector.getByRole("button", { name: /^Bus 4\b/ }).click();
await inspector.getByTestId("inspector-title").filter({ hasText: /^Bus 4$/ }).waitFor();
await page.getByRole("tab", { name: /^Buses/, selected: true }).waitFor();
await inspector.getByTestId("inspector-connected").getByText("Line 4–5 circuit 1").waitFor();
// Redispatch a generator too.
await page.getByRole("tab", { name: /Generators/ }).click();
await page.locator("table:visible tbody tr:not([data-spacer])").nth(1).click();
await inspector.getByRole("button", { name: "Set output" }).click();
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
// Save the study to a .stannum file: the edits, the saved solve and a
// fingerprint of the case file.
const { mkdtempSync, readFileSync, writeFileSync, copyFileSync } = await import("node:fs");
const { tmpdir } = await import("node:os");
const { createHash } = await import("node:crypto");
const studyDir = mkdtempSync(`${tmpdir()}/stannum-study-`);
const studyPath = `${studyDir}/case9-outage.stannum`;
await queue(studyPath);
await page.getByTestId("save-study").click();
await page.getByTestId("save-study").filter({ hasText: "case9-outage.stannum" }).waitFor();
await shot("study-saved");
const savedStudy = JSON.parse(await page.evaluate((p) => window.__files[p], studyPath));
assert.equal(savedStudy.format, "stannum-study");
assert.equal(savedStudy.edits.length, 2, "both study edits saved");
assert.equal(savedStudy.solves.length, 1, "the saved solve is kept");
assert.equal(savedStudy.solves[0].name, "Case as filed");
assert.equal(savedStudy.case.fingerprint.sha256, createHash("sha256").update(readFileSync(STANNUM_MATPOWER)).digest("hex"));
await page.getByRole("tab", { name: /Study/ }).click();
await page.getByRole("button", { name: "Clear all" }).click();
await page.getByTestId("study-badge").waitFor({ state: "detached" });

// Topology on a RAW v35 case: the Diagram's Network style, its tables and
// its layers.
await queue(STANNUM_RAW);
await page.getByRole("button", { name: "Open another…" }).click();
await page.getByRole("button", { name: "Diagram", exact: true }).click();
await page.getByRole("radio", { name: "Network" }).click();
await page.locator('[data-testid="network-view"] canvas').first().waitFor();
await page.getByRole("button", { name: "Tables", exact: true }).click();
const tables = page.getByTestId("tables-drawer");
await tables.getByRole("tab", { name: /Critical connections/ }).waitFor();
await tables.locator("table tbody tr").first().waitFor();
const firstBus = await tables.locator("table tbody tr td").first().innerText();
const busNumbers = await tables.locator("table tbody tr td:first-child").allInnerTexts();
await shot("diagram-tables");
// The shortest path between two buses, drawn as a layer.
await page.getByRole("button", { name: "Layers", exact: true }).click();
const layers = page.getByTestId("layers-panel");
await layers.getByLabel("From bus").fill(firstBus);
await layers.getByLabel("To bus").fill(busNumbers[Math.min(4, busNumbers.length - 1)]);
await layers.getByRole("button", { name: "Find" }).click();
await layers.getByTestId("path-result").filter({ hasText: /hop|No active AC path/ }).waitFor();
await shot("diagram-layers");

// Filters and colour rules narrow the tables and draw a legend.
await layers.getByRole("button", { name: /Filters and colours/ }).click();
await layers.getByLabel("Colour buses by").selectOption("base_kv");
await page.getByTestId("layer-legend").filter({ hasText: "Filters and colours" }).waitFor();
await layers.getByRole("button", { name: "Add filter" }).click();
await layers.getByLabel("Filter field").selectOption("number");
await layers.getByLabel("Filter operator").selectOption("eq");
await layers.getByLabel("Filter value").fill(firstBus);
await layers.getByTestId("filter-counts").filter({ hasText: /Showing 1 of/ }).waitFor();
await page.waitForFunction(() => document.querySelectorAll('[data-testid="tables-drawer"] table tbody tr').length === 1);
await layers.getByRole("button", { name: "Clear all" }).click();
await layers.getByLabel("Colour buses by").selectOption("");
await layers.getByRole("button", { name: "Hide layers" }).click();
await tables.getByRole("button", { name: "Hide tables" }).click();
await page.getByRole("radio", { name: "One-line" }).click();
await page.getByTestId("style-switch-status").waitFor({ state: "detached" });

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
// The dialog handler at the top answers yes: replace the network's one-line from above.
await page.getByRole("button", { name: "Generate", exact: true }).click();
await page.getByRole("dialog").waitFor({ state: "detached" });
await page.locator(".react-flow__node-bus").first().waitFor();
assert.equal(await page.locator(".react-flow__node-bus").count(), 5);
await page.locator("svg text", { hasText: /\d\.\d{3} pu/ }).first().waitFor();
await page.waitForTimeout(400); // fit-to-screen animation
await shot("diagram");

// One selection, seen in every module: pick a bus in Power flow, then follow
// the chip to Case diff and the diagram.
// Linking is opt-in: until it is on, a pick stays in its own module.
const chip = page.getByTestId("selection-chip");
// The chip's "Show in" links fold into a menu when the case bar is short of room.
async function showIn(target) {
  const direct = chip.getByRole("button", { name: `Show in ${target}` });
  if (await direct.count()) return direct.click();
  await chip.getByRole("button", { name: "Show in" }).click();
  await page.getByRole("menu", { name: "Show in" }).getByRole("menuitem", { name: target }).click();
}
await page.getByRole("button", { name: "Power flow", exact: true }).click();
await page.getByRole("tab", { name: /^Buses/ }).click();
const busRow = page.locator("table:visible tbody tr").nth(2);
const pickedBus = (await busRow.locator("td").first().innerText()).trim();
await busRow.click();
assert.equal(await chip.count(), 0, "unlinked selections stay local");
await page.getByRole("button", { name: "Link selection" }).click();
await busRow.click();
await chip.filter({ hasText: `Bus ${pickedBus}` }).waitFor();
await shot("selection-power");
const selectedFirstCell = () => page.locator("tr[aria-selected=true]:visible td").first().innerText();
await showIn("Case diff");
await page.locator("tr[aria-selected=true]:visible").first().waitFor();
assert.equal((await selectedFirstCell()).trim(), pickedBus);
await page.getByText("Original lines").waitFor();
await shot("selection-diff");
await showIn("Diagram");
await page.locator(".react-flow__node-bus.selected").waitFor();
assert.equal(await page.locator(".react-flow__node-bus.selected").count(), 1);
// The Diagram's tables show the same bus selected.
await page.getByRole("button", { name: "Tables", exact: true }).click();
await page.getByTestId("tables-drawer").locator("tr[aria-selected=true]").waitFor();
assert.equal((await page.getByTestId("tables-drawer").locator("tr[aria-selected=true] td").first().innerText()).trim(), pickedBus);
await shot("selection-diagram");
await page.getByTestId("tables-drawer").getByRole("button", { name: "Hide tables" }).click();

// Back the other way: a symbol picked in the diagram is selected in Power flow.
await page.locator(".react-flow__controls-fitview").click();
await page.waitForTimeout(400);
await page.locator(".react-flow__node-generator").first().click();
await chip.filter({ hasText: /Generator/ }).waitFor();
await showIn("Power flow");
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
// PDF export loads jsPDF on demand; check it still writes a PDF.
await queue("/tmp/labels.pdf");
await page.getByRole("button", { name: "Export", exact: true }).click();
await page.locator("div.fixed button").filter({ hasText: /^PDF$/ }).click();
await page.locator("div.fixed button").filter({ hasText: /^Export/ }).click();
await page.waitForFunction(() => window.__files["/tmp/labels.pdf"]);
assert((await page.evaluate(() => window.__files["/tmp/labels.pdf"])).startsWith("%PDF-"), "PDF export writes a PDF");
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
// A symbol from the case is inspected as its case element; the drawing's own
// fields, label reset included, are folded into the Drawing section.
await page.getByTestId("inspector").getByRole("button", { name: /^Drawing/, expanded: false }).click();
await page.getByRole("button", { name: "Reset label position" }).click();
assert.equal(await labelAt(), homeLabel, "reset puts the label back");
await page.getByTestId("inspector").getByRole("button", { name: /^Drawing/, expanded: true }).click();
await page.keyboard.press("Escape");

// A local drawing one hop around bus 4: the studied bus is highlighted and
// branches leaving the drawing end in markers. Double-clicking a marker brings
// that bus in without moving what is already drawn.
await page.getByRole("button", { name: "From case" }).click();
await page.getByLabel(/Around a bus/).check();
await page.getByRole("combobox", { name: "Bus number" }).fill("4");
await page.getByLabel("Hops out").selectOption("1");
// The dialog handler at the top answers yes: replace the edited drawing.
await page.getByRole("button", { name: "Generate", exact: true }).click();
await page.getByRole("dialog").waitFor({ state: "detached" });
await page.getByText(/^Around\b/).first().waitFor();
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

// The Network style draws the same neighbourhood as a bus-branch graph, and
// switching keeps the focus bus, the hop count and the selection.
const networkBus = (bus) =>
  // Cytoscape keeps its instance on the container. Tap the bus through it: a
  // mouse click could land on the selection card, wherever the layout put it.
  page.evaluate((b) => {
    const host = [...document.querySelectorAll('[data-testid="network-view"] div')].find((d) => d._cyreg);
    host._cyreg.cy.nodes().filter((n) => n.data("bus") === b).emit("tap");
  }, bus);
await page.getByRole("radio", { name: "Network" }).click();
const hopBar = page.getByTestId("network-hops");
await hopBar.filter({ hasText: "Around Bus 4" }).filter({ hasText: "1 hop" }).filter({ hasText: "3 buses" }).waitFor();
await page.locator('[data-testid="network-view"] canvas').first().waitFor();
await page.waitForTimeout(300);
await networkBus(5);
await page.getByTestId("network-selection").filter({ hasText: "Bus 5" }).waitFor();
await page.getByTestId("network-selection").getByTestId("element-inspector").filter({ hasText: "Network" }).waitFor();
// The snapshot leaves out RAW fields; opening the section fetches the bus's.
await page.getByTestId("network-selection").getByText(/RAW fields · line \d+/).click();
await page.getByTestId("source-fields").filter({ hasText: "BASKV" }).filter({ hasText: "Spur_69" }).waitFor();
await page.getByTestId("source-fields").evaluate((el) => el.scrollIntoView({ block: "end" }));
await shot("diagram-network-raw-fields");
await page.getByTestId("network-selection").getByText(/RAW fields · line \d+/).click();
await hopBar.getByRole("button", { name: "One hop more" }).click();
await hopBar.filter({ hasText: "2 hops" }).filter({ hasText: "5 buses" }).waitFor();
await shot("diagram-network");
await page.getByRole("radio", { name: "One-line" }).click();
await page.getByTestId("style-switch-status").waitFor({ state: "detached" });
await page.waitForFunction(() => document.querySelectorAll(".react-flow__node-bus").length === 5);
await page.locator("[aria-label=Hops]:visible", { hasText: "2 hops" }).waitFor();
await page.locator('.react-flow__node-bus.selected[data-id="b5"]').waitFor();
assert.equal(await page.locator(".react-flow__node.selected").count(), 1, "the bus bar, not its load, is selected");
await page.waitForTimeout(600); // fit, then centre on the selection
await shot("diagram-network-oneline");
// And back: the network keeps the redrawn focus and the selection.
await page.getByRole("radio", { name: "Network" }).click();
await hopBar.filter({ hasText: "Around Bus 4" }).filter({ hasText: "2 hops" }).waitFor();
await page.getByTestId("network-selection").filter({ hasText: "Bus 5" }).waitFor();
// "Draw as one-line" from the selection redraws around it.
await page.getByTestId("network-selection").getByRole("button", { name: "Draw as one-line" }).click();
await page.getByRole("radio", { name: "One-line", checked: true }).waitFor();
await page.getByTestId("style-switch-status").waitFor({ state: "detached" });
await page.locator('.react-flow__node-bus.selected[data-id="b5"]').waitFor();
await page.locator("[aria-label=Hops]:visible", { hasText: "2 hops" }).waitFor();

// Hand drawing: a wire released on a bus bar, between its taps, gets a new tap.
// The dialog handler at the top answers yes.
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
// The dialog handler at the top answers yes.
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
// Dragged off the bar it stays wired; dragged back on, it snaps on again
// and its wire moves to the new spot rather than a second one being added.
await page.mouse.move(onBus.x + onBus.width / 2, onBus.y + 40);
await page.mouse.down();
await page.mouse.move(onBus.x + onBus.width / 2 + 40, onBus.y + 200, { steps: 10 });
await page.mouse.up();
const loadY = async (ok) => {
  for (let i = 0; i < 30 && !ok((await load.boundingBox()).y); i++) await page.waitForTimeout(100);
  return (await load.boundingBox()).y;
};
assert((await loadY((y) => y > onBus.y + 100)) > onBus.y + 100, "load dragged off the bar");
const offBus = await load.boundingBox();
await page.mouse.move(offBus.x + offBus.width / 2, offBus.y + 40);
await page.mouse.down();
await page.mouse.move(barNow.x + 100, barNow.y + barNow.height + 40, { steps: 10 });
await page.mouse.up();
const back = await loadY((y) => Math.abs(y - (barNow.y + barNow.height)) < 3);
assert(Math.abs(back - (barNow.y + barNow.height)) < 3, `load dragged back snaps onto the bar again (${back})`);
// The wire moves to its new tap a couple of frames after the drop.
await page.waitForTimeout(300);
assert.equal(await page.locator(".react-flow__edge").count(), 2, "re-attached load keeps one wire");
await page.waitForFunction(() =>
  [...document.querySelectorAll(".react-flow__edge-path:not(.react-flow__edge-interaction)")].every((p) => p.getAttribute("d").split("L").length === 2)
);

// Rotation: R turns the selection a quarter. A turned bus runs vertically;
// symbols dropped beside it turn to face it and sit on it, and a wire released
// on the bar gets a tap on the side it comes from.
// The dialog handler at the top answers yes.
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
  // The dialog handler at the top answers yes: replace the edited drawing.
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
    // The dialog handler at the top answers yes.
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

  // A three-winding transformer is one symbol, not its internal star bus
  // with a two-winding symbol per winding. One hop around bus 3004 reaches
  // it through bus 3002; its other two windings end in a marker each.
  await page.getByRole("button", { name: "From case" }).click();
  await page.getByLabel(/Around a bus/).check();
  await page.getByRole("combobox", { name: "Bus number" }).fill("3004");
  await page.getByLabel("Hops out").selectOption("1");
  // The dialog handler at the top answers yes.
  await page.getByRole("button", { name: "Generate", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "detached" });
  const t3 = page.locator(".react-flow__node-transformer3");
  await t3.first().waitFor();
  assert.equal(await t3.count(), 1, "one three-winding symbol around bus 3004");
  assert.match(await t3.innerText(), /T 3002-3001-3011/, "named by its windings' buses");
  assert.equal(await page.locator('.react-flow__node[data-id="b100003"]').count(), 0, "star bus not drawn");
  assert.equal(await page.locator('.react-flow__node[data-id="o51"], .react-flow__node[data-id="o52"]').count(), 2, "a marker per winding leaving the drawing");
  assert.doesNotMatch(await page.getByTestId("voltage-legend").innerText(), /(^|\s)1(,| kV)/, "no 1 kV star bus in the legend");
  await page.locator(".react-flow__controls-fitview").click();
  await page.waitForTimeout(400);
  await shot("diagram-three-winding");

  // Power flow lists every DC line; with Q limits on (the default), the case converges and
  // the line-commutated converters absorb reactive power. Two-terminal lines
  // use the detailed converter model: the rectifier holds the order.
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  assert.equal(await page.getByLabel("Enforce generator Q limits").isChecked(), true, "Q limits on by default");
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
  // The dialog handler at the top answers yes: replace the edited drawing.
  await page.getByRole("button", { name: "Generate", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "detached" });
  await page.waitForTimeout(400); // fit-to-screen animation
  const dc = page.locator(".react-flow__node-dcLink");
  assert.equal(await dc.count(), 1, "one DC line drawn");
  await dc.click();
  await chip.filter({ hasText: /DC line East link/ }).waitFor();
  await shot("dc-diff-picked");
  await showIn("Case diff");
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
  await showIn("Diagram");
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
  await tab.getByRole("gridcell", { name: "Offshore", exact: true }).first().waitFor();
  await tab.getByRole("gridcell", { name: "DC slack" }).waitFor();
  await tab.getByRole("gridcell", { name: "Droop" }).first().waitFor();
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
  // The count says how many of the whole table are listed.
  await page.getByText(new RegExp(`^${violations} rows? of [\\d,]+$`)).waitFor();
  const first = page.locator("table tbody tr:visible").first();
  assert.equal((await first.locator("td").first().innerText()).trim(), worst.replace("Bus ", ""), "worst first");
  await first.click();
  await page.getByTestId("table-selection").filter({ hasText: worst }).waitFor();
  await page.getByTestId("power-inspector").getByTestId("inspector-headline").filter({ hasText: /below|above/ }).waitFor();
  await shot("findings-violations");

  await page.getByTestId("power-inspector").getByRole("button", { name: /Draw around/ }).click();
  await page.locator(".react-flow__node-bus.selected").first().waitFor();
  await page.getByText(/^Around\b/).first().waitFor();
  const studied = page.locator(".react-flow__node-bus").filter({ has: page.locator("title", { hasText: "Studied bus" }) });
  assert.equal(await studied.count(), 1, "drawn around the worst bus");
  await page.waitForTimeout(600); // centred after the fit
  await shot("findings-drawn");

  // The diagram inspects the same element as Power flow did: its results and
  // case data first, the drawing's own fields folded underneath.
  const drawnInspector = page.getByTestId("inspector");
  await drawnInspector.getByTestId("inspector-title").filter({ hasText: worst }).waitFor();
  await drawnInspector.getByTestId("inspector-headline").filter({ hasText: /below|above/ }).waitFor();
  const drawingSection = drawnInspector.getByTestId("inspector-drawing").getByRole("button", { name: /Drawing/ });
  assert.equal(await drawingSection.getAttribute("aria-expanded"), "false", "drawing fields folded");
  await shot("inspector-diagram");
  await drawingSection.click();
  await drawnInspector.getByTestId("inspector-drawing").getByText(/Bus|Label|Name/).first().waitFor();
  await drawingSection.click();

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
  // Half-width panes keep their chrome inside them: tabs and toolbar buttons
  // that don't fit move into "More" menus instead of spilling over.
  const spills = await page.evaluate(() =>
    [...document.querySelectorAll("[data-pane] [role=tablist], [data-pane] [data-testid=diagram-toolbar]")]
      .filter((el) => el.scrollWidth > el.clientWidth + 1)
      .map((el) => el.getAttribute("role") ?? el.getAttribute("data-testid"))
  );
  assert.deepEqual(spills, [], "nothing overflows a half-width pane");
  // The selected bus's inspector floats over the drawing rather than taking a column.
  const floatBox = await page.locator("[data-pane=left] [data-testid=inspector]").boundingBox();
  const canvasBox = await page.locator("[data-pane=left] .react-flow").boundingBox();
  assert(floatBox.x < canvasBox.x + canvasBox.width, "the inspector floats over the drawing in a narrow pane");
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
  await page.getByTestId("power-inspector").getByRole("button", { name: /Draw around/ }).click();
  await page.getByText(/^Around\b/).filter({ hasText: /Line|Transformer/ }).first().waitFor();
  await page.getByTestId("draw-request").waitFor({ state: "detached" });
  await page.getByTestId("focus-halo").first().waitFor();
  assert.equal(await studied.count(), 0, "a branch has no studied bus");
  await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
  await page.locator(".react-flow__controls-fitview").click();
  await page.waitForTimeout(400);
  await shot("around-branch");

  // An empty drawing offers to draw around the worst violation.
  // The dialog handler at the top answers yes.
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

  // MATPOWER has no RAW records, so the topology is built from the power flow model.
  await page.getByRole("button", { name: "Diagram", exact: true }).click();
  await page.getByRole("button", { name: "Tables", exact: true }).click();
  await page.getByTestId("tables-drawer").getByRole("tab", { name: /Diagnostics/ }).click();
  await page.getByText("Built from the power flow model.").waitFor();
  await shot("tables-matpower");
  await page.getByTestId("tables-drawer").getByRole("button", { name: "Hide tables" }).click();
}

// Records: every record type of the open case with every field, sorted,
// filtered, and picked into the inspector and onto the diagram.
await queue(STANNUM_RAW);
await page.getByRole("button", { name: "Open another…" }).click();
await page.getByRole("button", { name: "Records", exact: true }).click();
const recordTypes = page.getByTestId("record-types");
await recordTypes.getByRole("button", { name: /^Buses/ }).waitFor();
const records = page.getByTestId("records-table");
const recordRows = records.locator("tbody tr:not([data-spacer])");
await recordRows.first().waitFor();
assert.equal(await recordRows.count(), 5, "five buses");
for (const field of ["I", "NAME", "BASKV", "IDE", "AREA", "ZONE", "VM", "VA", "EVLO"])
  await records.getByRole("columnheader", { name: field, exact: true }).waitFor();
// Sort by base kV, highest first.
const kvHeader = records.getByRole("columnheader", { name: "BASKV", exact: true });
await kvHeader.click();
await kvHeader.click();
const kvIndex = await records.locator("thead th").evaluateAll((ths) => ths.findIndex((th) => th.textContent.trim() === "BASKV"));
const kvs = (await recordRows.locator(`td:nth-child(${kvIndex + 1})`).allInnerTexts()).map(Number);
assert.deepEqual(kvs, [...kvs].sort((a, b) => b - a), "sorted by base kV, highest first");
await shot("records-buses");
// A filter chip on any field.
const recordChips = page.getByTestId("records-module").getByTestId("filter-chips");
await recordChips.getByRole("button", { name: "Add filter" }).click();
const recordEditor = page.getByRole("dialog", { name: "Edit filter" });
await recordEditor.getByLabel("Filter field").selectOption({ label: "BASKV" });
await recordEditor.getByLabel("Filter operator").selectOption("lte");
await recordEditor.getByLabel("Filter value").fill("100");
await recordEditor.getByRole("button", { name: "Done" }).click();
await recordChips.getByTestId("filter-chip").filter({ hasText: "BASKV ≤ 100" }).waitFor();
await recordChips.getByText(/^\d+ of 5 buses$/).waitFor();
for (const kv of await recordRows.locator(`td:nth-child(${kvIndex + 1})`).allInnerTexts()) assert.equal(kv.trim(), "69");
await shot("records-filtered");
await recordChips.getByRole("button", { name: /^Remove filter/ }).click();
// Loads, and a pick that opens the inspector, then Draw around.
await recordTypes.getByRole("button", { name: /^Loads/ }).click();
await records.getByRole("columnheader", { name: "PL", exact: true }).waitFor();
await recordRows.first().click();
const recordInspector = page.getByTestId("records-inspector");
await recordInspector.getByTestId("inspector-title").filter({ hasText: /^Load / }).waitFor();
await shot("records-inspector");
// Edit a load's MW, apply it, and see the inspector read the new value.
const plIndex = await records.locator("thead th").evaluateAll((ths) => ths.findIndex((th) => th.textContent.trim() === "PL"));
// Double-clicking a record not yet picked opens the inspector between the
// two clicks, which narrows the columns: the cell clicked first is edited.
await recordInspector.getByRole("button", { name: "Close inspector" }).click();
await recordInspector.waitFor({ state: "detached" });
await recordRows.nth(1).locator(`td:nth-child(${plIndex + 1}) span`).dblclick();
await recordRows.nth(1).locator(`td:nth-child(${plIndex + 1})`).getByLabel("Cell value").waitFor();
await shot("records-double-click");
await records.getByLabel("Cell value").press("Escape");
await recordRows.first().click();
await recordRows.first().locator(`td:nth-child(${plIndex + 1}) span`).dblclick();
await records.getByLabel("Cell value").fill("abc");
await records.getByLabel("Cell value").press("Enter");
assert.equal(await records.getByLabel("Cell value").getAttribute("aria-invalid"), "true", "a non-number is refused");
await records.getByLabel("Cell value").fill("95.5");
await records.getByLabel("Cell value").press("Enter");
const editBar = page.getByTestId("records-edits");
await editBar.getByText("1 change not applied").waitFor();
await records.locator("[data-pending]").filter({ hasText: "95.5" }).waitFor();
await shot("records-edit-pending");
await editBar.getByRole("button", { name: "Apply" }).click();
await editBar.getByText("1 edited value not saved to a file").waitFor();
await recordRows.first().locator(`td:nth-child(${plIndex + 1})`).filter({ hasText: /^95\.5$/ }).waitFor();
await recordRows.first().click();
await recordInspector.getByText("95.50").first().waitFor();
await shot("records-edit-applied");
// Save as writes the edited case to a new file, which becomes the open case.
const savedCase = `${(await import("node:os")).tmpdir()}/five_bus_edited_${Date.now()}.raw`;
await queue(savedCase);
await editBar.getByRole("button", { name: "Save as…" }).click();
await editBar.waitFor({ state: "detached" });
await page.getByText(/five_bus_edited_\d+\.raw/).first().waitFor();
const savedText = (await import("node:fs")).readFileSync(savedCase, "utf8");
assert.match(savedText, /\n {4}2,'1 ',1, {3}1, {3}1, {6}95\.5, {4}30\.000,/, "only the edited field changed");
await recordInspector.getByRole("button", { name: "Draw around" }).click();
await page.locator(".react-flow__node-bus").first().waitFor();
// A MATPOWER case lists the power flow model's tables.
await queue(STANNUM_MATPOWER);
await page.getByRole("button", { name: "Open another…" }).click();
await page.getByRole("button", { name: "Records", exact: true }).click();
await recordTypes.getByRole("button", { name: /^Generators/ }).waitFor();
await records.getByRole("columnheader", { name: "BUS_I", exact: true }).waitFor();
// Rows arrive a page at a time from the core, after the header.
await recordRows.nth(8).waitFor();
assert.equal(await recordRows.count(), 9, "nine buses in case9");
await shot("records-matpower");

// Open the study again after the case moved beside it and changed: the
// case is found, the change is pointed out, and the edits and saved solve
// come back.
const movedCase = `${studyDir}/case9.m`;
copyFileSync(STANNUM_MATPOWER, movedCase);
writeFileSync(movedCase, readFileSync(movedCase, "utf8") + "\n% edited after the study was saved\n");
const movedStudy = `${studyDir}/moved.stannum`;
const moved = { ...savedStudy, case: { ...savedStudy.case, path: `${studyDir}/gone/case9.m`, relative: "case9.m" } };
await page.evaluate(([p, c]) => (window.__files[p] = c), [movedStudy, JSON.stringify(moved)]);
await queue(movedStudy);
await page.getByRole("button", { name: "Open another…" }).click();
const studyNotice = page.getByTestId("study-notice");
await studyNotice.getByText("Opened moved.stannum").waitFor();
await studyNotice.getByText(/The case was found at .*case9\.m/).waitFor();
await studyNotice.getByText(/case9\.m has changed since the study was saved/).waitFor();
await page.getByTestId("study-badge").filter({ hasText: "2 edits" }).waitFor();
await page.getByTestId("save-study").filter({ hasText: "moved.stannum" }).waitFor();
await page.getByText("Converged").first().waitFor();
assert.equal(await page.getByTestId("save-study").getByLabel("unsaved changes").count(), 0, "a study just opened has nothing to save");
await page.getByTestId("study-badge").click();
await page.getByTestId("study-edits").getByText("Line 4–5 circuit 1 out of service").waitFor();
assert.equal(await page.getByTestId("saved-solves").getByRole("textbox").first().inputValue(), "Case as filed");
await shot("study-reopened");
// Closing a changed study asks first; then the empty screen lists it.
await page.getByRole("button", { name: "Clear all" }).click();
await page.getByTestId("study-badge").waitFor({ state: "detached" });
const asked = dialogs.length;
await page.getByRole("button", { name: "Close case" }).click();
await page.getByText("No case open.").waitFor();
assert.match(dialogs[asked] ?? "", /moved\.stannum has changes that aren't saved/);
const recent = page.getByTestId("recent-files").first();
await recent.getByText("moved.stannum").waitFor();
await recent.getByText("case9.m").first().waitFor();
await shot("recent-files");

// Case diff of the working case: a study outage on a RAW case shows as the
// branch record's status changing from the file.
await queue(STANNUM_RAW);
await page.getByRole("button", { name: "Open case…" }).first().click();
await page.getByRole("button", { name: "Solve", exact: true }).click();
await page.getByText(/Converged|Iteration limit/).first().waitFor();
await page.getByRole("tab", { name: /Branches/ }).click();
await page.locator("table:visible tbody tr:not([data-spacer])").first().click();
await inspector.getByRole("button", { name: "Take out" }).click();
await page.getByTestId("study-badge").filter({ hasText: "1 edit" }).click();
await page.getByRole("button", { name: "Compare with file" }).click();
await page.getByText("five_bus.raw (working)").waitFor();
await page.getByText("1 differing records").waitFor();
await page.getByRole("button", { name: /^BRANCH/ }).click();
await page.locator("section:has-text('Records') table tbody tr").first().click();
await page.getByLabel("Changed fields only").check();
const statRow = page.locator("section:has-text('Fields') table tbody tr", { hasText: /^STAT/ });
await statRow.waitFor();
assert.deepEqual(
  (await statRow.locator("td").allInnerTexts()).map((t) => t.trim()).slice(0, 3),
  ["STAT", "1", "0"],
  "the outage sets the branch's STAT from 1 to 0"
);
await shot("diff-working");

// Run files: Record keeps what you do as a run file, and Run plays one back.
{
  const palette = page.getByRole("dialog", { name: "Search and commands" });
  const command = async (text, option) => {
    await page.keyboard.press("Control+k");
    await palette.getByRole("combobox").fill(text);
    await palette.getByRole("option", { name: option }).click();
  };
  const runDir = mkdtempSync(`${tmpdir()}/stannum-run-`);
  copyFileSync(STANNUM_MATPOWER, `${runDir}/case9.m`);
  await queue(`${runDir}/case9.m`);
  await page.getByRole("button", { name: /Open (case|another)…/ }).first().click();
  await page.locator("[data-case-summary]").getByText("case9.m").waitFor();
  await command("record", /Record a run file/);
  const bar = page.getByTestId("recording-bar");
  await bar.getByText("1 step so far.").waitFor();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  await page.locator("text=Converged >> visible=true").first().waitFor();
  await page.getByRole("tab", { name: /Branches/ }).click();
  await page.locator("table:visible tbody tr:not([data-spacer])").nth(1).click();
  await inspector.getByRole("button", { name: "Take out" }).click();
  await page.getByTestId("study-badge").filter({ hasText: "1 edit" }).waitFor();
  // The edit, then the solve that follows it.
  await bar.getByText("4 steps so far.").waitFor();
  await page.getByRole("tab", { name: "Overview" }).click();
  await shot("run-file-recording");
  await page.keyboard.press("Control+k");
  await palette.getByRole("combobox").fill("run file");
  await palette.getByRole("option", { name: /Stop recording and save run file/ }).waitFor();
  await shot("run-file-palette");
  await page.keyboard.press("Escape");
  const runFile = `${runDir}/case9_outage.stnrun`;
  await queue(runFile);
  await page.getByTestId("save-recording").click();
  await bar.waitFor({ state: "detached" });
  const recorded = readFileSync(runFile, "utf8");
  assert.match(recorded, /^# stannum-run-file version 1\n/, recorded);
  assert.match(recorded, /^open_case path=case9\.m$/m, recorded);
  assert.match(recorded, /^set_study_edits edits=\[\{"element": "line:4:5:1", "kind": "status", "online": false\}\]$/m, recorded);
  // The app's solve settings are the defaults, so the solves read plainly.
  assert.equal(recorded.match(/^solve_power_flow$/gm)?.length, 2, recorded);

  // Played back on an empty window, it opens the case, takes the line out
  // and solves, and the app shows all of it.
  await page.getByRole("button", { name: "Close case" }).click();
  await page.getByText("No case open.").waitFor();
  await queue(runFile);
  await command("run file", /Run a run file/);
  await page.getByTestId("run-file-notice").getByText(/^Ran case9_outage\.stnrun: 4 steps in \d+ ms\.$/).waitFor();
  await page.locator("[data-case-summary]").getByText("case9.m").waitFor();
  await page.getByTestId("study-badge").filter({ hasText: "1 edit" }).waitFor();
  await page.locator("text=Converged >> visible=true").first().waitFor();
  await shot("run-file-ran");

  // A line that fails stops the run and says which; a misspelled command
  // stops the file before anything runs.
  writeFileSync(`${runDir}/broken.stnrun`, "open_case path=case9.m\nelement_detail element=bus:99\nsolve_power_flow\n");
  await queue(`${runDir}/broken.stnrun`);
  await command("run file", /Run a run file/);
  const notice = page.getByTestId("run-file-notice");
  await notice.getByText("broken.stnrun stopped: a line failed, 1 not run.").waitFor();
  await notice.getByText("Line 2, element_detail: bus:99 isn't in this case.").waitFor();
  await shot("run-file-failed");
  writeFileSync(`${runDir}/typo.stnrun`, "open_case path=case9.m\nsolve_powerflow\n");
  await queue(`${runDir}/typo.stnrun`);
  await command("run file", /Run a run file/);
  await notice.getByText("The run file didn't run.").waitFor();
  await notice.getByText(/Line 2: Stannum has no command called solve_powerflow\./).waitFor();
  await notice.getByRole("button", { name: "Dismiss" }).click();
}

// A bug in the core: the command that hit it fails with a notice, the log
// has the details, and the window carries on. Save diagnostics writes the
// log and versions, without the case.
if (STANNUM_CRASH_TEST) {
  // case9.m is still open from the run file above.
  await page.locator("[data-case-summary]").getByText("case9.m").waitFor();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();
  await page.evaluate(() => (window.__faultNext = "solve_power_flow"));
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  const crash = page.getByTestId("crash-notice");
  await crash.getByText("Something went wrong inside Stannum: solving power flow stopped.").waitFor();
  await crash.getByText("Internal error in fault_test: fault injected into fault_test for testing. This is a bug in Stannum; please report it.").waitFor();
  await shot("crash-notice");

  // Still usable: the same solve works, and so do the other modules.
  await page.getByRole("button", { name: "Solve", exact: true }).click();
  await page.locator("text=Converged >> visible=true").first().waitFor();
  await page.getByRole("button", { name: "Records", exact: true }).click();
  await page.getByTestId("records-table").locator("tbody tr:not([data-spacer])").first().waitFor();
  await page.getByRole("button", { name: "Power flow", exact: true }).click();

  await crash.getByRole("button", { name: "Show log" }).click();
  const lines = page.getByRole("log", { name: "Log lines" });
  await lines.getByText(/Internal error.* at .*crash\.rs:\d+: fault injected into fault_test for testing/).first().waitFor();
  await shot("crash-log");

  const saved = "/tmp/stannum-diagnostics.txt";
  await queue(saved);
  await crash.getByRole("button", { name: "Save diagnostics…" }).click();
  await lines.getByText("Saved diagnostics. Attach the file when you report the problem.").waitFor();
  const report = await page.evaluate((p) => window.__files[p], saved);
  assert.match(report, /^Stannum diagnostics\n/, report);
  assert.match(report, /Core: +stannum-core \d+\.\d+\.\d+, command API \d+/, report);
  assert.match(report, /Solvers: +rs-power \S+, nodal \S+/, report);
  assert.match(report, /Internal errors in the core: +[1-9]/, report);
  assert.match(report, /ERROR \[core\] Internal error.*: fault injected into fault_test for testing/, report);
  assert.match(report, /ERROR \[app\] Solving power flow failed after .*: Internal error in fault_test/, report);
  // The log, not the case: none of the case file's data rows are in it.
  assert.doesNotMatch(report, /mpc\.(bus|gen|branch) = \[/, report);
  await crash.getByRole("button", { name: "Dismiss" }).click();
  await crash.waitFor({ state: "detached" });
}

// Keyboard: modules by Ctrl+number, tables by arrows, tabs by arrows, the
// shortcut list, and focus going back where it was after the palette.
{
  await queue(STANNUM_RAW);
  await page.getByRole("button", { name: /Open (case|another)…/ }).first().click();
  await page.locator("[data-case-summary]").getByText("five_bus.raw").waitFor();
  await page.keyboard.press("Control+3");
  const recordsPane = page.locator('[data-module="records"]');
  await recordsPane.getByTestId("record-types").getByRole("button", { name: /^Buses/ }).click();
  const grid = recordsPane.getByRole("grid");
  await grid.locator("tbody tr:not([data-spacer])").first().waitFor();
  await grid.focus();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  const cursorRow = grid.locator("tr[data-cursor]");
  assert.equal(await cursorRow.getAttribute("aria-rowindex"), "4", "two rows down from the first");
  assert.equal(await grid.getAttribute("aria-activedescendant"), await cursorRow.getAttribute("id"));
  await page.keyboard.press("Enter");
  await page.getByTestId("records-inspector").getByTestId("inspector-title").filter({ hasText: /^Bus / }).waitFor();
  await shot("keyboard-records-row");
  // F2 edits the row's first editable field; Escape gives the keyboard back to the table.
  await grid.focus();
  await page.keyboard.press("F2");
  const editor = grid.getByRole("textbox", { name: "Cell value" });
  await editor.waitFor();
  await page.keyboard.press("Escape");
  await editor.waitFor({ state: "detached" });
  await page.waitForFunction(() => document.activeElement?.getAttribute("role") === "grid");
  // Up from the first row reaches the headers; Enter sorts.
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  await grid.locator('th[aria-sort="ascending"]').waitFor();
  await page.keyboard.press("ArrowDown");
  await page.waitForFunction(() => document.activeElement?.getAttribute("role") === "grid");

  // Power flow tabs: one Tab stop, arrows switch.
  await page.keyboard.press("Control+2");
  const powerPane = page.locator('[data-module="power"]');
  await powerPane.getByRole("button", { name: "Solve", exact: true }).click();
  await powerPane.locator("text=Converged >> visible=true").first().waitFor();
  const results = powerPane.getByRole("tablist", { name: "Power flow results" });
  await results.getByRole("tab", { selected: true }).focus();
  const before = await results.getByRole("tab", { selected: true }).textContent();
  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(
    (was) => document.activeElement?.getAttribute("role") === "tab" && document.activeElement.getAttribute("aria-selected") === "true" && document.activeElement.textContent !== was,
    before
  );
  assert.equal(await results.locator('[role="tab"][tabindex="0"]').count(), 1, "one tab in the Tab order");
  await shot("keyboard-tabs");

  // "?" lists the shortcuts; Escape closes it and focus comes back.
  await page.keyboard.press("?");
  const shortcuts = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await shortcuts.getByText("Move between rows").waitFor();
  await shot("keyboard-shortcuts");
  await page.keyboard.press("Escape");
  await shortcuts.waitFor({ state: "detached" });
  await page.waitForFunction(() => document.activeElement?.getAttribute("role") === "tab");
  await page.keyboard.press("Control+k");
  await page.getByRole("dialog", { name: "Search and commands" }).waitFor();
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => document.activeElement?.getAttribute("role") === "tab");
}

assert.deepEqual(errors, [], `page errors: ${errors.join("; ")}`);
await browser.close();
console.log("smoke test passed");
