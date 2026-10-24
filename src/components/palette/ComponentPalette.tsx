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

// Previews drawn with the same geometry as the canvas symbols, scaled down.
const INK = "#1e293b";
const SW = 1.5;

function Preview({ w, h, children }: { w: number; h: number; children: React.ReactNode }) {
  const scale = 0.62;
  return (
    <svg width={w * scale} height={h * scale} viewBox={`-1 -1 ${w + 2} ${h + 2}`} fill="none" stroke={INK} strokeWidth={SW}>
      {children}
    </svg>
  );
}

function BusSymbol() {
  return (
    <svg width={44} height={20}>
      <rect x={2} y={7} width={40} height={6} fill="#ca8a04" rx={1} />
    </svg>
  );
}

function GeneratorSymbol() {
  return (
    <Preview w={40} h={56}>
      <path d="M20,0 V20" />
      <circle cx={20} cy={36} r={16} fill="white" />
      <path d="M11,36 q4.5,-9 9,0 t9,0" />
    </Preview>
  );
}

function LoadSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V24" />
      <polygon points="11,24 29,24 20,60" fill={INK} strokeLinejoin="round" />
    </Preview>
  );
}

function MotorSymbol() {
  return (
    <Preview w={40} h={56}>
      <path d="M20,0 V22" />
      <circle cx={20} cy={38} r={16} fill="white" />
      <text x={20} y={42.5} textAnchor="middle" fontSize={13} fontWeight={600} fill={INK} stroke="none">
        M
      </text>
    </Preview>
  );
}

function CapacitorSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V22 M9,22 H31 M9,28 H31 M20,28 V48 M12,48 H28 M15,52 H25 M18,56 H22" />
    </Preview>
  );
}

function ReactorSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V16 a5,4 0 1,1 0,8 a5,4 0 1,1 0,8 a5,4 0 1,1 0,8 V48 M12,48 H28 M15,52 H25 M18,56 H22" />
    </Preview>
  );
}

function TransformerSymbol() {
  return (
    <Preview w={40} h={100}>
      <path d="M20,0 V22 M20,78 V100" />
      <circle cx={20} cy={38} r={16} />
      <circle cx={20} cy={62} r={16} />
    </Preview>
  );
}

function CBSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V20 M20,40 V60" />
      <rect x={10} y={20} width={20} height={20} fill={INK} />
    </Preview>
  );
}

function DisconnectorSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V18 M15,18 H25 M20,42 L32,21 M20,42 V60" />
      <circle cx={20} cy={42} r={2} fill={INK} />
    </Preview>
  );
}

function FuseSymbol() {
  return (
    <Preview w={40} h={60}>
      <path d="M20,0 V60" />
      <rect x={14} y={18} width={12} height={24} />
    </Preview>
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
    title: "Compensation",
    items: [
      { type: "shunt", label: "Capacitor", symbol: <CapacitorSymbol /> },
      { type: "shunt", label: "Reactor", symbol: <ReactorSymbol /> },
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
    if (entry.label === "Reactor") {
      Object.assign(data, { label: "Reactor", nominalMVAR: -25 });
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
