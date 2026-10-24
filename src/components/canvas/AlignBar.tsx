import {
  AlignStartVertical,
  AlignCenterVertical,
  AlignEndVertical,
  AlignStartHorizontal,
  AlignCenterHorizontal,
  AlignEndHorizontal,
  AlignHorizontalDistributeCenter,
  AlignVerticalDistributeCenter,
} from "lucide-react";
import { useDiagramStore } from "../../store/diagramStore";
import type { AlignMode } from "../../store/types";

const GROUPS: { how: AlignMode; icon: React.ReactNode; title: string; min: number }[][] = [
  [
    { how: "left", icon: <AlignStartVertical size={15} />, title: "Align left edges", min: 2 },
    { how: "center", icon: <AlignCenterVertical size={15} />, title: "Align centres", min: 2 },
    { how: "right", icon: <AlignEndVertical size={15} />, title: "Align right edges", min: 2 },
  ],
  [
    { how: "top", icon: <AlignStartHorizontal size={15} />, title: "Align tops", min: 2 },
    { how: "middle", icon: <AlignCenterHorizontal size={15} />, title: "Align middles", min: 2 },
    { how: "bottom", icon: <AlignEndHorizontal size={15} />, title: "Align bottoms", min: 2 },
  ],
  [
    { how: "distribute-x", icon: <AlignHorizontalDistributeCenter size={15} />, title: "Space evenly across", min: 3 },
    { how: "distribute-y", icon: <AlignVerticalDistributeCenter size={15} />, title: "Space evenly down", min: 3 },
  ],
];

/** Floating align and distribute buttons, shown while two or more symbols are selected. */
export function AlignBar() {
  const count = useDiagramStore((s) => s.selectedNodeIds.length);
  const align = useDiagramStore((s) => s.align);
  if (count < 2) return null;
  return (
    <div
      role="toolbar"
      aria-label="Align selection"
      className="absolute top-3 left-1/2 -translate-x-1/2 z-10 flex items-center gap-0.5 px-1.5 py-1
                 rounded-lg border border-slate-200 bg-white shadow-sm"
    >
      <span className="text-[11px] text-slate-500 px-1.5">{count} selected</span>
      {GROUPS.map((group, i) => (
        <div key={i} className="flex items-center gap-0.5 pl-1 ml-1 border-l border-slate-200">
          {group.map((a) => (
            <button
              key={a.how}
              onClick={() => align(a.how)}
              disabled={count < a.min}
              title={a.title}
              aria-label={a.title}
              className={`p-1.5 rounded text-slate-600 ${count < a.min ? "opacity-30 cursor-not-allowed" : "hover:bg-slate-100 cursor-pointer"}`}
            >
              {a.icon}
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
