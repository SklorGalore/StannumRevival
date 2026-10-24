import type { DiagramMetadata } from "../../store/types";

function inlineStyles(el: SVGElement) {
  const computed = window.getComputedStyle(el);
  const relevant = [
    "fill", "stroke", "stroke-width", "font-size", "font-family",
    "font-weight", "text-anchor", "dominant-baseline",
  ];
  let inline = "";
  for (const prop of relevant) {
    const val = computed.getPropertyValue(prop);
    if (val) inline += `${prop}:${val};`;
  }
  if (inline) el.setAttribute("style", inline);
  for (const child of Array.from(el.children)) {
    inlineStyles(child as SVGElement);
  }
}

export function exportSVG(
  flowContainer: HTMLElement,
  metadata: DiagramMetadata
): string {
  const original = flowContainer.querySelector(
    ".react-flow__viewport"
  ) as SVGGElement | null;

  if (!original) throw new Error("React Flow viewport not found");

  // Build a clean standalone SVG
  const bbox = (
    flowContainer.querySelector(".react-flow__renderer") as HTMLElement
  )?.getBoundingClientRect() ?? { width: 1200, height: 800 };

  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  svg.setAttribute("width", String(Math.round(bbox.width)));
  svg.setAttribute("height", String(Math.round(bbox.height)));
  svg.setAttribute(
    "viewBox",
    `0 0 ${Math.round(bbox.width)} ${Math.round(bbox.height)}`
  );

  // White background
  const bg = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  bg.setAttribute("width", "100%");
  bg.setAttribute("height", "100%");
  bg.setAttribute("fill", "white");
  svg.appendChild(bg);

  // Title
  const titleEl = document.createElementNS("http://www.w3.org/2000/svg", "title");
  titleEl.textContent = metadata.title;
  svg.appendChild(titleEl);

  // Clone viewport content
  const clone = original.cloneNode(true) as SVGGElement;
  inlineStyles(clone);
  // Remove React Flow UI overlays that may have leaked in
  clone
    .querySelectorAll(
      ".react-flow__controls, .react-flow__minimap, .react-flow__attribution, .react-flow__handle"
    )
    .forEach((el) => el.remove());

  svg.appendChild(clone);

  return new XMLSerializer().serializeToString(svg);
}
