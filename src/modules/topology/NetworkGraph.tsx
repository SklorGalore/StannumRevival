import { useEffect, useRef } from "react";
import { useTheme } from "../../store/themeStore";
import cytoscape, { type Core, type StylesheetJson } from "cytoscape";
// @ts-expect-error: cytoscape-fcose ships no type declarations.
import fcose from "cytoscape-fcose";

cytoscape.use(fcose);
import type { GraphData, TopoPath, TopologySnapshot } from "../../lib/analysis/types";
import { nodeForBus } from "./viewModel";

export interface GraphSelection {
  bus: number | null;
  equipment: string | null;
}

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
  { selector: "node.articulation", style: { "underlay-color": colors.finding, "underlay-opacity": 0.48, "underlay-padding": 5 } },
  { selector: "edge.bridge", style: { "underlay-color": colors.finding, "underlay-opacity": 0.65, "underlay-padding": 4 } },
  { selector: "node.selected", style: { "border-color": colors.selection, "border-width": 4 } },
  { selector: "edge.selected", style: { "line-outline-color": colors.selection, "line-outline-width": 2 } },
  { selector: "node.path", style: { "background-color": colors.path } },
  { selector: "edge.path", style: { "line-color": colors.path, width: 4 } },
  { selector: "node.selected, node.path, node.articulation", style: { "min-zoomed-font-size": 0 } },
  { selector: "node.hovered", style: { "border-style": "dashed", "border-width": 3 } },
  { selector: "edge.hovered", style: { width: 5 } },
  { selector: "node.label-hidden", style: { label: "" } },
  // Colour rules from the Filters & colours panel. Path and selection still win.
  // Coloured buses are a little larger and outlined so pale colours still read.
  { selector: "node[fill]", style: { "background-color": "data(fill)", width: 38, height: 38, "border-color": colors.outline, "border-width": 1.5 } },
  { selector: "node.junction[fill]", style: { width: 18, height: 18 } },
  { selector: "edge[stroke]", style: { "line-color": "data(stroke)", width: 2.2 } },
  // Under a colour rule, critical buses and bridges get a ring instead of a
  // filled halo, so the halo doesn't hide the colours.
  { selector: "node.articulation[fill]", style: { "underlay-opacity": 0, "border-color": colors.finding, "border-width": 3.5 } },
  { selector: "edge.bridge[stroke]", style: { "underlay-opacity": 0.25, "underlay-padding": 3 } },
  { selector: "node.path", style: { "background-color": colors.path } },
  { selector: "edge.path", style: { "line-color": colors.path } },
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
    const important = node.hasClass("selected") || node.hasClass("path") || node.hasClass("articulation");
    node.toggleClass("label-hidden", hide && !important);
  });
}

export function NetworkGraph({
  graph,
  snapshot,
  path,
  selection,
  onInspect,
  visible,
  fitSignal,
  colors,
}: {
  graph: GraphData;
  snapshot: TopologySnapshot;
  path: TopoPath | null;
  selection: GraphSelection;
  onInspect: (s: GraphSelection) => void;
  visible: boolean;
  fitSignal: number;
  colors?: { nodeColors: Record<string, string>; edgeColors: Record<string, string> };
}) {
  const container = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const inspect = useRef(onInspect);
  inspect.current = onInspect;
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
    const bridges = new Set(snapshot.analysis.bridges.map((e) => e.id));
    const cuts = new Set(snapshot.analysis.articulation_buses);
    const syntheticCuts = new Set(snapshot.analysis.synthetic_articulations);
    const pathNodes = new Set(path?.node_ids ?? []);
    const pathEdges = new Set(path?.connection_ids ?? []);
    const kinds = new Map(snapshot.case.equipment.map((e) => [e.id, e.kind]));
    const cy = cytoscape({
      container: container.current,
      elements: [
        ...graph.nodes.map((n) => ({
          data: {
            id: n.id,
            label: n.bus === null ? "T" : n.member_buses.length > 1 ? `${n.bus} · ${n.member_buses.length}` : String(n.bus),
            bus: n.bus,
            equipment: n.equipment_id,
          },
          classes: [
            n.bus === null ? "junction" : "bus",
            (n.bus !== null && cuts.has(n.bus)) || syntheticCuts.has(n.id) ? "articulation" : "",
            pathNodes.has(n.id) ? "path" : "",
          ].filter(Boolean).join(" "),
        })),
        ...graph.edges.map((e) => ({
          data: { id: e.id, source: e.source, target: e.target, equipment: e.equipment_id },
          classes: [kinds.get(e.equipment_id) ?? "line", bridges.has(e.id) ? "bridge" : "", pathEdges.has(e.id) ? "path" : ""]
            .filter(Boolean)
            .join(" "),
        })),
      ],
      style: styleFor(palette.current),
      layout: layoutFor(graph.nodes.length),
      minZoom: 0.08,
      maxZoom: 4,
      wheelSensitivity: 0.25,
    });
    stretchToFit(cy, container.current);
    cy.on("tap", "node", (ev) => inspect.current({ bus: ev.target.data("bus") ?? null, equipment: ev.target.data("equipment") ?? null }));
    cy.on("tap", "edge", (ev) => inspect.current({ bus: null, equipment: ev.target.data("equipment") }));
    cy.on("mouseover", "node, edge", (ev) => ev.target.addClass("hovered"));
    cy.on("mouseout", "node, edge", (ev) => ev.target.removeClass("hovered"));
    cy.on("zoom", () => refreshLabels(cy));
    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [graph, snapshot, path]);

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
    refreshLabels(cy);
  }, [selection, graph, snapshot, path]);

  // Colour rules recolour in place, without a new layout.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.batch(() => {
      cy.nodes().forEach((n) => {
        const c = colors?.nodeColors[n.id()];
        if (c) n.data("fill", c);
        else n.removeData("fill");
      });
      cy.edges().forEach((e) => {
        const c = colors?.edgeColors[e.id()];
        if (c) e.data("stroke", c);
        else e.removeData("stroke");
      });
    });
  }, [colors, graph, snapshot, path]);

  // The pane may have been laid out while hidden; refit when it is shown.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !visible) return;
    cy.resize();
    cy.fit(undefined, 35);
  }, [visible, fitSignal]);

  // Cytoscape restyles its container, so give it a full-size child of a positioned wrapper.
  return (
    <div className="absolute inset-0">
      <div ref={container} className="w-full h-full" />
    </div>
  );
}
