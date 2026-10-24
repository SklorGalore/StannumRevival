import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { SLDNodeType } from "../../store/types";
import { defaultsByType } from "../../store/defaults";

interface PaletteEntry {
  type: SLDNodeType;
  label: string;
  symbol: React.ReactNode;
}

interface PaletteCategory {
  title: string;
  items: PaletteEntry[];
}

function BusSymbol() {
  return (
    <svg width={44} height={20}>
      <rect x={2} y={8} width={40} height={6} fill="#ca8a04" stroke="#1e293b" strokeWidth={1} rx={1} />
    </svg>
  );
}

function GeneratorSymbol() {
  return (
    <svg width={30} height={36}>
      <line x1={15} y1={0} x2={15} y2={6} stroke="#1e293b" strokeWidth={1.5} />
      <circle cx={15} cy={18} r={12} fill="white" stroke="#1e293b" strokeWidth={1.5} />
      <text x={15} y={23} textAnchor="middle" fontSize={12} fontWeight="bold" fill="#1e293b">G</text>
      <line x1={15} y1={30} x2={15} y2={36} stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

function LoadSymbol() {
  return (
    <svg width={28} height={32}>
      <line x1={14} y1={0} x2={14} y2={8} stroke="#1e293b" strokeWidth={1.5} />
      <polygon points="14,8 2,28 26,28" fill="#f1f5f9" stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

function MotorSymbol() {
  return (
    <svg width={30} height={36}>
      <line x1={15} y1={0} x2={15} y2={6} stroke="#1e293b" strokeWidth={1.5} />
      <circle cx={15} cy={20} r={14} fill="white" stroke="#1e293b" strokeWidth={1.5} />
      <text x={15} y={25} textAnchor="middle" fontSize={13} fontWeight="bold" fill="#1e293b">M</text>
    </svg>
  );
}

function TransformerSymbol() {
  return (
    <svg width={30} height={52}>
      <line x1={15} y1={0} x2={15} y2={6} stroke="#1e293b" strokeWidth={1.5} />
      <circle cx={15} cy={18} r={12} fill="white" stroke="#1e293b" strokeWidth={1.5} />
      <circle cx={15} cy={38} r={12} fill="white" stroke="#1e293b" strokeWidth={1.5} />
      <line x1={15} y1={50} x2={15} y2={52} stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

function CBSymbol() {
  return (
    <svg width={16} height={30}>
      <line x1={8} y1={0} x2={8} y2={8} stroke="#1e293b" strokeWidth={1.5} />
      <rect x={1} y={8} width={14} height={14} fill="#1e293b" stroke="#1e293b" strokeWidth={1} />
      <line x1={8} y1={22} x2={8} y2={30} stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

function DisconnectorSymbol() {
  return (
    <svg width={16} height={30}>
      <line x1={8} y1={0} x2={8} y2={10} stroke="#1e293b" strokeWidth={1.5} strokeDasharray="3,2" />
      <circle cx={8} cy={10} r={2} fill="#1e293b" />
      <circle cx={8} cy={20} r={2} fill="#1e293b" />
      <line x1={8} y1={20} x2={16} y2={12} stroke="#1e293b" strokeWidth={1.5} />
      <line x1={8} y1={20} x2={8} y2={30} stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

function FuseSymbol() {
  return (
    <svg width={16} height={30}>
      <line x1={8} y1={0} x2={8} y2={8} stroke="#1e293b" strokeWidth={1.5} />
      <rect x={1} y={8} width={14} height={14} fill="white" stroke="#1e293b" strokeWidth={1.5} rx={2} />
      <circle cx={8} cy={15} r={2} fill="#1e293b" />
      <line x1={8} y1={22} x2={8} y2={30} stroke="#1e293b" strokeWidth={1.5} />
    </svg>
  );
}

const CATEGORIES: PaletteCategory[] = [
  {
    title: "Generation",
    items: [
      { type: "generator", label: "Generator", symbol: <GeneratorSymbol /> },
    ],
  },
  {
    title: "Network",
    items: [
      { type: "bus", label: "Bus", symbol: <BusSymbol /> },
    ],
  },
  {
    title: "Transformation",
    items: [
      { type: "transformer", label: "Transformer", symbol: <TransformerSymbol /> },
    ],
  },
  {
    title: "Protection",
    items: [
      { type: "circuitBreaker", label: "Circuit Breaker", symbol: <CBSymbol /> },
      { type: "disconnector", label: "Disconnector", symbol: <DisconnectorSymbol /> },
      { type: "fuse", label: "Fuse", symbol: <FuseSymbol /> },
    ],
  },
  {
    title: "Load",
    items: [
      { type: "load", label: "Load (Static)", symbol: <LoadSymbol /> },
      {
        type: "load",
        label: "Load (Motor)",
        symbol: <MotorSymbol />,
      },
    ],
  },
];

function PaletteItem({ entry }: { entry: PaletteEntry }) {
  const onDragStart = (event: React.DragEvent) => {
    const data = { ...defaultsByType[entry.type] };
    // Motor loads get loadType override
    if (entry.type === "load" && entry.label.includes("Motor")) {
      (data as { loadType?: string }).loadType = "motor";
    }
    event.dataTransfer.setData("application/sld-node-type", entry.type);
    event.dataTransfer.setData(
      "application/sld-node-data",
      JSON.stringify(data)
    );
    event.dataTransfer.effectAllowed = "move";
  };

  return (
    <div
      draggable
      onDragStart={onDragStart}
      className="flex flex-col items-center gap-1 p-2 rounded cursor-grab
                 hover:bg-slate-100 active:cursor-grabbing select-none
                 transition-colors"
    >
      <div className="flex items-center justify-center h-14">
        {entry.symbol}
      </div>
      <span className="text-xs text-slate-600 text-center leading-tight">
        {entry.label}
      </span>
    </div>
  );
}

function CategorySection({ category }: { category: PaletteCategory }) {
  const [open, setOpen] = useState(true);

  return (
    <div className="border-b border-slate-100 last:border-0">
      <button
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center justify-between px-3 py-2
                   text-xs font-semibold text-slate-500 uppercase tracking-wider
                   hover:bg-slate-50 transition-colors"
      >
        {category.title}
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
      </button>
      {open && (
        <div className="grid grid-cols-2 gap-1 px-2 pb-2">
          {category.items.map((item, i) => (
            <PaletteItem key={i} entry={item} />
          ))}
        </div>
      )}
    </div>
  );
}

export function ComponentPalette() {
  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-slate-200">
        <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
          Components
        </p>
        <p className="text-xs text-slate-400 mt-0.5">Drag onto canvas</p>
      </div>
      <div className="flex-1 overflow-y-auto">
        {CATEGORIES.map((cat) => (
          <CategorySection key={cat.title} category={cat} />
        ))}
      </div>
    </div>
  );
}
