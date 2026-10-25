import { useRef, useState } from "react";
import { X, Download } from "lucide-react";
import { useDiagramStore } from "../../store/diagramStore";
import { exportSVG } from "../../lib/export/svgExporter";
import { useTheme } from "../../store/themeStore";
import { exportPNG } from "../../lib/export/pngExporter";
import { exportPDF } from "../../lib/export/pdfExporter";
import { showSaveDialog, saveBinaryFile } from "../../lib/tauri/commands";

interface Props {
  onClose: () => void;
}

type Format = "svg" | "png" | "pdf";
type DPI = 96 | 150 | 300;

function btoa_utf8(str: string): string {
  return btoa(
    encodeURIComponent(str).replace(/%([0-9A-F]{2})/g, (_, p1) =>
      String.fromCharCode(parseInt(p1, 16))
    )
  );
}

export function ExportDialog({ onClose }: Props) {
  const [format, setFormat] = useState<Format>("png");
  const [dpi, setDpi] = useState<DPI>(150);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const metadata = useDiagramStore((s) => s.metadata);

  const flowContainerRef = useRef<HTMLElement | null>(null);

  async function handleExport() {
    setExporting(true);
    setError(null);

    try {
      const container =
        document.querySelector(".react-flow") as HTMLElement | null;
      if (!container) throw new Error("Canvas not found");

      const ext = format === "svg" ? "svg" : format === "png" ? "png" : "pdf";
      const safeName = metadata.title.replace(/[^a-zA-Z0-9_-]/g, "_");
      const path = await showSaveDialog(`${safeName}.${ext}`, [
        { name: ext.toUpperCase(), extensions: [ext] },
      ]);
      if (!path) {
        setExporting(false);
        return;
      }

      // Exports are always light and carry the full result text: redraw the
      // canvas that way for the capture.
      const theme = useTheme.getState();
      let svgString: string;
      theme.setExportLight(true);
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      try {
        svgString = exportSVG(container, metadata);
      } finally {
        theme.setExportLight(false);
      }

      if (format === "svg") {
        const b64 = btoa_utf8(svgString);
        await saveBinaryFile(path, b64);
      } else if (format === "png") {
        const pngB64 = await exportPNG(svgString, dpi);
        await saveBinaryFile(path, pngB64);
      } else {
        const pngB64 = await exportPNG(svgString, dpi);
        const pdfBytes = exportPDF(pngB64, metadata);
        const b64 = btoa(
          Array.from(pdfBytes)
            .map((b) => String.fromCharCode(b))
            .join("")
        );
        await saveBinaryFile(path, b64);
      }

      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="bg-white rounded-lg shadow-xl w-80 p-5">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-slate-700">Export Diagram</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">
            <X size={16} />
          </button>
        </div>

        <div className="mb-3">
          <label className="block text-xs font-medium text-slate-500 mb-1">
            Format
          </label>
          <div className="flex gap-2">
            {(["svg", "png", "pdf"] as Format[]).map((f) => (
              <button
                key={f}
                onClick={() => setFormat(f)}
                className={`flex-1 py-1.5 text-xs rounded border transition-colors
                  ${format === f
                    ? "border-teal-500 bg-teal-50 text-teal-800 font-semibold"
                    : "border-slate-200 text-slate-600 hover:bg-slate-50"
                  }`}
              >
                {f.toUpperCase()}
              </button>
            ))}
          </div>
        </div>

        {format !== "svg" && (
          <div className="mb-4">
            <label className="block text-xs font-medium text-slate-500 mb-1">
              Resolution
            </label>
            <div className="flex gap-2">
              {([96, 150, 300] as DPI[]).map((d) => (
                <button
                  key={d}
                  onClick={() => setDpi(d)}
                  className={`flex-1 py-1.5 text-xs rounded border transition-colors
                    ${dpi === d
                      ? "border-teal-500 bg-teal-50 text-teal-800 font-semibold"
                      : "border-slate-200 text-slate-600 hover:bg-slate-50"
                    }`}
                >
                  {d} DPI
                </button>
              ))}
            </div>
          </div>
        )}

        {error && (
          <p className="text-xs text-red-600 mb-3 bg-red-50 px-2 py-1.5 rounded">
            {error}
          </p>
        )}

        <div className="flex gap-2 mt-4">
          <button
            onClick={onClose}
            className="flex-1 py-1.5 text-xs border border-slate-200 rounded text-slate-600 hover:bg-slate-50"
          >
            Cancel
          </button>
          <button
            onClick={handleExport}
            disabled={exporting}
            className="flex-1 py-1.5 text-xs rounded bg-teal-700 text-white font-medium
                       hover:bg-teal-800 disabled:opacity-50 flex items-center justify-center gap-1.5"
          >
            <Download size={12} />
            {exporting ? "Exporting…" : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
