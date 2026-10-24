import jsPDF from "jspdf";
import type { DiagramMetadata } from "../../store/types";

export function exportPDF(
  pngBase64: string,
  metadata: DiagramMetadata,
  orientation: "landscape" | "portrait" = "landscape",
  pageSize: "a4" | "a3" | "letter" = "a3"
): Uint8Array {
  const doc = new jsPDF({ orientation, unit: "mm", format: pageSize, compress: true });

  const pw = doc.internal.pageSize.getWidth();
  const ph = doc.internal.pageSize.getHeight();
  const titleBlockH = 22;
  const margin = 5;
  const drawH = ph - titleBlockH - margin * 2;

  // Diagram image, scaled to fit the drawing area without stretching.
  const image = `data:image/png;base64,${pngBase64}`;
  const { width: iw, height: ih } = doc.getImageProperties(image);
  const drawW = pw - margin * 2;
  const k = Math.min(drawW / iw, drawH / ih);
  doc.addImage(image, "PNG", margin + (drawW - iw * k) / 2, margin + (drawH - ih * k) / 2, iw * k, ih * k);

  // Title block
  const tbY = ph - titleBlockH - margin;
  doc.setLineWidth(0.4);
  doc.rect(margin, tbY, pw - margin * 2, titleBlockH);

  // Vertical divider
  doc.line(pw - 60, tbY, pw - 60, tbY + titleBlockH);

  doc.setFontSize(14);
  doc.setFont("helvetica", "bold");
  doc.text(metadata.title, margin + 3, tbY + 9);

  doc.setFontSize(8);
  doc.setFont("helvetica", "normal");
  if (metadata.company) {
    doc.text(metadata.company, margin + 3, tbY + 16);
  }
  if (metadata.description) {
    doc.text(metadata.description.slice(0, 60), margin + 3, tbY + 20);
  }

  doc.setFontSize(8);
  doc.text(`Rev: ${metadata.revision}`, pw - 57, tbY + 7);
  doc.text(
    `Date: ${new Date(metadata.modifiedAt).toLocaleDateString()}`,
    pw - 57,
    tbY + 13
  );
  doc.text("Stannum", pw - 57, tbY + 19);

  return doc.output("arraybuffer") as unknown as Uint8Array;
}
