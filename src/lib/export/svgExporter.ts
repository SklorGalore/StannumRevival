import type { DiagramMetadata } from "../../store/types";

const NS = "http://www.w3.org/2000/svg";
const MARGIN = 24;
const STYLE_PROPS = [
  "fill",
  "stroke",
  "stroke-width",
  "stroke-dasharray",
  "opacity",
  "font-size",
  "font-family",
  "font-weight",
  "text-anchor",
  "dominant-baseline",
];

/** Deep-clone an SVG element, copying the computed styles of the live
 *  element onto the copy (a detached clone has no computed style). */
function cloneWithStyles(source: Element): Element {
  const copy = source.cloneNode(true) as Element;
  const walk = (from: Element, to: Element) => {
    const computed = window.getComputedStyle(from);
    let style = "";
    for (const prop of STYLE_PROPS) {
      const value = computed.getPropertyValue(prop);
      if (value) style += `${prop}:${value};`;
    }
    to.setAttribute("style", style);
    to.removeAttribute("class");
    for (let i = 0; i < from.children.length; i++) walk(from.children[i], to.children[i]);
  };
  walk(source, copy);
  return copy;
}

/** `translate(12px, 34px)` → [12, 34]. */
function translation(el: HTMLElement): [number, number] {
  const m = /translate\(\s*(-?[\d.]+)px,\s*(-?[\d.]+)px\)/.exec(el.style.transform);
  return m ? [parseFloat(m[1]), parseFloat(m[2])] : [0, 0];
}

/**
 * Build a standalone SVG of the whole drawing (not just what's on screen).
 * React Flow renders symbols as HTML boxes that each hold an SVG, so the
 * symbols are re-assembled into one SVG at their diagram positions.
 */
export function exportSVG(flowContainer: HTMLElement, metadata: DiagramMetadata): string {
  const content = document.createElementNS(NS, "g");
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const grow = (x: number, y: number, w: number, h: number) => {
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x + w);
    maxY = Math.max(maxY, y + h);
  };

  // Wires: already in diagram coordinates.
  for (const edge of flowContainer.querySelectorAll<SVGGElement>(".react-flow__edge")) {
    const box = edge.getBBox();
    if (box.width || box.height) grow(box.x, box.y, box.width, box.height);
    const copy = document.createElementNS(NS, "g");
    for (const child of Array.from(edge.children)) {
      // Skip React Flow's wide transparent hit area.
      if (child.classList.contains("react-flow__edge-interaction")) continue;
      if (child.classList.contains("segment-grips")) continue;
      copy.appendChild(cloneWithStyles(child));
    }
    content.appendChild(copy);
  }

  // Symbols: one SVG per node, placed at the node's position.
  for (const node of flowContainer.querySelectorAll<HTMLElement>(".react-flow__node")) {
    const symbol = node.querySelector<SVGSVGElement>(":scope > .node-wrapper > svg, :scope svg");
    if (!symbol) continue;
    const [x, y] = translation(node);
    const box = symbol.getBBox();
    grow(x + box.x, y + box.y, box.width, box.height);
    const g = document.createElementNS(NS, "g");
    g.setAttribute("transform", `translate(${x},${y})`);
    const copy = cloneWithStyles(symbol);
    for (const child of Array.from(copy.childNodes)) g.appendChild(child);
    content.appendChild(g);
  }

  if (!Number.isFinite(minX)) throw new Error("The diagram is empty.");
  const width = Math.ceil(maxX - minX + MARGIN * 2);
  const height = Math.ceil(maxY - minY + MARGIN * 2);
  content.setAttribute("transform", `translate(${MARGIN - minX},${MARGIN - minY})`);

  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("xmlns", NS);
  svg.setAttribute("width", String(width));
  svg.setAttribute("height", String(height));
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  const title = document.createElementNS(NS, "title");
  title.textContent = metadata.title;
  svg.appendChild(title);
  const bg = document.createElementNS(NS, "rect");
  bg.setAttribute("width", "100%");
  bg.setAttribute("height", "100%");
  bg.setAttribute("fill", "white");
  svg.appendChild(bg);
  svg.appendChild(content);
  return new XMLSerializer().serializeToString(svg);
}
