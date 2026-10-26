import { useEffect, useRef } from "react";
import { useTheme } from "../../store/themeStore";
import cytoscape, { type Core, type StylesheetJson } from "cytoscape";
// @ts-expect-error: cytoscape-fcose ships no type declarations.
import fcose from "cytoscape-fcose";

cytoscape.use(fcose);
import type { GraphData, TopologySnapshot } from "../../lib/analysis/types";
import type { LayerMarks, Mark } from "../../lib/diagram/layers";
import { nodeForBus, type GraphSelection } from "./viewModel";
import { useLog } from "../../store/logStore";
import { duration } from "../../lib/log/log";

export type { GraphSelection };

// Palette and styling follow nodal's graph-presentation.ts (light theme).
const LIGHT = {
  node: "#5b7f91",
  text: "#233c50",
  surface: "#ffffff",
  junction: "#78909f",
  edge: "#94a8b5",
  finding: "#986315",
  selection: "#0f766e",
  path: "#2563eb",
  outline: "#334155",
};

const DARK: typeof LIGHT = {
  node: "#7fa3b5",
  text: "#d5e3ec",
  surface: "#151c2b",
  junction: "#8fa6b4",
  edge: "#5c7282",
  finding: "#d39a3d",
  selection: "#2dd4bf",
  path: "#60a5fa",
  outline: "#cbd5e1",
};

const styleFor = (colors: typeof LIGHT): StylesheetJson => [
  { selector: "node", style: { "background-color": colors.node, width: 25, height: 25, label: "data(label)", color: colors.text, "font-size": 11, "text-valign": "bottom", "text-margin-y": 6, "border-width": 2, "border-color": colors.surface, "min-zoomed-font-size": 9 } },
  { selector: "node.junction", style: { shape: "diamond", "background-color": colors.surface, "border-color": colors.junction, width: 18, height: 18 } },
  { selector: "edge", style: { width: 1.7, "line-color": colors.edge, "curve-style": "bezier" } },
  { selector: "edge.transformer", style: { "line-style": "dashed" } },
  { selector: "edge.switch", style: { "line-style": "dotted" } },
  { selector: "node.selected", style: { "border-color": colors.selection, "border-width": 4 } },
  { selector: "edge.selected", style: { "line-outline-color": colors.selection, "line-outline-width": 2 } },
  { selector: "node[halo], node.bold", style: { "min-zoomed-font-size": 0 } },
  { selector: "node.hovered", style: { "border-style": "dashed", "border-width": 3 } },
  { selector: "edge.hovered", style: { width: 5 } },
  { selector: "node.label-hidden", style: { label: "" } },
  // Layers (lib/diagram/layers.ts): fills and strokes, rings, and paths.
  // Coloured buses are a little larger and outlined so pale colours still read.
  { selector: "node[fill]", style: { "background-color": "data(fill)", width: 32, height: 32, "border-color": colors.outline, "border-width": 1.5 } },
  { selector: "node.junction[fill]", style: { width: 18, height: 18 } },
  { selector: "edge[stroke]", style: { "line-color": "data(stroke)", width: 2.2 } },
  { selector: "node[halo]", style: { "underlay-color": "data(halo)", "underlay-opacity": 0.48, "underlay-padding": 5 } },
  { selector: "edge[halo]", style: { "underlay-color": "data(halo)", "underlay-opacity": 0.6, "underlay-padding": 4 } },
  // Over a colour, a ring instead of a filled halo, so the colour still shows.
  { selector: "node[halo][fill]", style: { "underlay-opacity": 0, "border-color": "data(halo)", "border-width": 3.5 } },
  { selector: "edge[halo][stroke]", style: { "underlay-opacity": 0.3, "underlay-padding": 3 } },
  { selector: "edge.bold", style: { width: 4 } },
  { selector: "node.selected", style: { "border-color": colors.selection, "border-width": 4 } },
  // The bus a neighbourhood is drawn around (Diagram's Network style).
  { selector: "node.focus", style: { "underlay-color": colors.selection, "underlay-opacity": 0.22, "underlay-padding": 9, "underlay-shape": "ellipse", "min-zoomed-font-size": 0, "font-weight": "bold" } },
];

/**
 * fCoSE spring layout with room between buses: springs a few bus-widths
 * long, strong repulsion, and labels counted in each bus's footprint.
 * The panel's aspect ratio is passed in so the result fills a wide view.
 */
function layoutFor(nodes: number): cytoscape.LayoutOptions {
  return {
    name: "fcose",
    quality: nodes > 600 ? "draft" : "default",
    animate: false,
    randomize: true,
    padding: 30,
    nodeDimensionsIncludeLabels: true,
    idealEdgeLength: () => 100,
    nodeRepulsion: () => 25000,
    edgeElasticity: () => 0.45,
    nodeSeparation: 90,
    gravity: 0.2,
    gravityRange: 3.8,
    numIter: 2500,
    tile: true,
    packComponents: true,
  } as cytoscape.LayoutOptions;
}

/**
 * Spring layouts come out roughly round, while the view is wide. Spread the
 * buses horizontally (up to 2.2x) so the graph uses the whole panel.
 */
function stretchToFit(cy: Core, el: HTMLElement) {
  const box = cy.nodes().boundingBox({});
  if (!box.w || !box.h || !el.clientWidth || !el.clientHeight) return;
  const factor = Math.min(2.2, el.clientWidth / el.clientHeight / (box.w / box.h));
  if (factor <= 1.05) return;
  const cx = box.x1 + box.w / 2;
  cy.nodes().positions((n) => ({ x: cx + (n.position("x") - cx) * factor, y: n.position("y") }));
  cy.fit(undefined, 30);
}

function refreshLabels(cy: Core) {
  const hide = cy.zoom() < 0.6;
  cy.nodes().forEach((node) => {
    const important = node.hasClass("selected") || node.hasClass("bold") || !!node.data("halo") || node.hasClass("focus");
    node.toggleClass("label-hidden", hide && !important);
  });
}

export function NetworkGraph({
  graph,
  snapshot,
  selection,
  onInspect,
  visible,
  fitSignal,
  marks,
  focus,
  onRecentre,
}: {
  graph: GraphData;
  snapshot: TopologySnapshot;
  selection: GraphSelection;
  onInspect: (s: GraphSelection) => void;
  visible: boolean;
  fitSignal: number;
  /** What the Diagram's layers mark (see lib/diagram/layers.ts). */
  marks: LayerMarks;
  /** A bus to mark as the centre of the view. */
  focus?: number | null;
  /** Double-clicking a bus asks to centre the view on it. */
  onRecentre?: (bus: number) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const inspect = useRef(onInspect);
  inspect.current = onInspect;
  const recentre = useRef(onRecentre);
  recentre.current = onRecentre;
  const dark = useTheme((s) => s.theme === "dark");
  const palette = useRef(dark ? DARK : LIGHT);
  palette.current = dark ? DARK : LIGHT;
  // Restyle in place when the theme changes, without re-running the layout.
  useEffect(() => {
    cyRef.current?.style(styleFor(dark ? DARK : LIGHT));
  }, [dark]);

  // Rebuild only when the displayed graph changes.
  useEffect(() => {
    if (!container.current) return;
    const kinds = new Map(snapshot.case.equipment.map((e) => [e.id, e.kind]));
    const laidOut = performance.now();
    const cy = cytoscape({
      container: container.current,
      elements: [
        ...graph.nodes.map((n) => ({
          data: {
            id: n.id,
            label: n.bus === null ? "T" : n.member_buses.length > 1 ? `${n.bus} · ${n.member_buses.length}` : String(n.bus),
            bus: n.bus,
            members: n.member_buses?.length ? n.member_buses : n.bus !== null ? [n.bus] : [],
            equipment: n.equipment_id,
          },
          classes: n.bus === null ? "junction" : "bus",
        })),
        ...graph.edges.map((e) => ({
          data: { id: e.id, source: e.source, target: e.target, equipment: e.equipment_id },
          classes: kinds.get(e.equipment_id) ?? "line",
        })),
      ],
      style: styleFor(palette.current),
      layout: layoutFor(graph.nodes.length),
      minZoom: 0.08,
      maxZoom: 4,
      wheelSensitivity: 0.25,
    });
    useLog
      .getState()
      .add("info", `Laid out the topology graph: ${graph.nodes.length} nodes, ${graph.edges.length} edges (${duration(performance.now() - laidOut)})`);
    stretchToFit(cy, container.current);
    cy.on("tap", "node", (ev) => inspect.current({ bus: ev.target.data("bus") ?? null, equipment: ev.target.data("equipment") ?? null }));
    cy.on("tap", "edge", (ev) => inspect.current({ bus: null, equipment: ev.target.data("equipment") }));
    cy.on("dbltap", "node", (ev) => {
      const bus = ev.target.data("bus");
      if (bus !== null && bus !== undefined) recentre.current?.(bus);
    });
    cy.on("mouseover", "node, edge", (ev) => ev.target.addClass("hovered"));
    cy.on("mouseout", "node, edge", (ev) => ev.target.removeClass("hovered"));
    cy.on("zoom", () => refreshLabels(cy));
    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [graph, snapshot]);

  // Selection highlights without re-layout.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.elements().removeClass("selected");
    if (selection.bus !== null) {
      const node = nodeForBus(graph, selection.bus);
      if (node) cy.getElementById(node.id).addClass("selected");
    }
    if (selection.equipment !== null) {
      cy.edges().filter((e) => e.data("equipment") === selection.equipment).addClass("selected");
      cy.nodes().filter((n) => n.data("equipment") === selection.equipment).addClass("selected");
    }
    cy.nodes().removeClass("focus");
    if (focus !== null && focus !== undefined) {
      const node = nodeForBus(graph, focus);
      if (node) cy.getElementById(node.id).addClass("focus");
    }
    refreshLabels(cy);
  }, [selection, focus, graph, snapshot, marks]);

  // Layers restyle in place, without a new layout.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    const set = (el: cytoscape.SingularElementArgument, key: string, value: string | undefined) =>
      value ? el.data(key, value) : el.removeData(key);
    cy.batch(() => {
      cy.nodes().forEach((n) => {
        const members: number[] = n.data("members") ?? [];
        const m: Mark | undefined = members.length ? members.map((b) => marks.buses.get(b)).find(Boolean) : marks.nodes.get(n.id());
        set(n, "fill", m?.color);
        set(n, "halo", m?.halo);
        n.toggleClass("bold", !!m?.bold);
      });
      cy.edges().forEach((e) => {
        const m = marks.elements.get(e.data("equipment"));
        set(e, "stroke", m?.color);
        set(e, "halo", m?.halo);
        e.toggleClass("bold", !!m?.bold);
      });
    });
    refreshLabels(cy);
  }, [marks, graph, snapshot]);

  // The pane may have been laid out while hidden; refit when it is shown.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !visible) return;
    cy.resize();
    cy.fit(undefined, 35);
  }, [visible, fitSignal]);

  // Follow the container's size (a side panel opening, a split pane being
  // dragged), keeping the middle of the view where it was.
  useEffect(() => {
    const el = container.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    let last = { w: el.clientWidth, h: el.clientHeight };
    const observer = new ResizeObserver(() => {
      const cy = cyRef.current;
      const next = { w: el.clientWidth, h: el.clientHeight };
      if (!cy || !next.w || !next.h || (next.w === last.w && next.h === last.h)) return;
      // A pane coming out of hiding is refitted by the effect above instead.
      if (last.w && last.h) {
        cy.resize();
        cy.panBy({ x: (next.w - last.w) / 2, y: (next.h - last.h) / 2 });
      }
      last = next;
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Cytoscape restyles its container, so give it a full-size child of a positioned wrapper.
  return (
    <div className="absolute inset-0">
      <div ref={container} className="w-full h-full" />
    </div>
  );
}
