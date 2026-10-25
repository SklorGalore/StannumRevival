import type { ReactNode } from "react";
import { Wand2 } from "lucide-react";
import { useWorkspace, type ModuleId } from "../store/workspaceStore";
import { elementLabel, parseElement } from "../lib/elements";

/** The bus a one-line is drawn around for an element: the bus itself or its first terminal. */
export function centreBus(element: string): number | null {
  return parseElement(element)?.buses[0] ?? null;
}

/**
 * "Draw around this": opens the diagram drawn a few hops around an element.
 * Renders nothing when the open case can't be drawn.
 */
export function DrawAroundButton({
  element,
  from,
  label = "Draw around",
  compact = false,
  className = "",
}: {
  element: string;
  from: ModuleId;
  label?: string;
  /** Icon only. */
  compact?: boolean;
  className?: string;
}) {
  const canDraw = useWorkspace((s) => !!s.caseSummary?.power.available);
  const drawAround = useWorkspace((s) => s.drawAround);
  if (!canDraw || centreBus(element) === null) return null;
  const title = `Draw a one-line around ${elementLabel(element)}`;
  return (
    <button
      type="button"
      className={`shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium text-teal-700 hover:bg-teal-50 hover:text-teal-900 ${className}`}
      onClick={(e) => {
        e.stopPropagation();
        drawAround(element, from);
      }}
      title={title}
      aria-label={compact ? title : undefined}
    >
      <Wand2 size={12} />
      {!compact && label}
    </button>
  );
}

/** An element name that selects it when clicked, followed by a "Draw around" action. */
export function ElementLink({
  element,
  from,
  onOpen,
  children,
}: {
  element: string;
  from: ModuleId;
  /** Show the element where it is listed. */
  onOpen: () => void;
  children?: ReactNode;
}) {
  return (
    <span className="inline-flex items-center gap-1">
      <button
        type="button"
        className="font-medium text-teal-800 underline decoration-teal-300 underline-offset-2 hover:decoration-teal-700"
        onClick={(e) => {
          e.stopPropagation();
          onOpen();
        }}
        title={`Show ${elementLabel(element)}`}
      >
        {children ?? elementLabel(element)}
      </button>
      <DrawAroundButton element={element} from={from} compact />
    </span>
  );
}
