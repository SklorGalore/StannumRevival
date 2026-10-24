export function exportPNG(
  svgString: string,
  dpi: 96 | 150 | 300 = 150
): Promise<string> {
  const scale = dpi / 96;

  return new Promise((resolve, reject) => {
    const blob = new Blob([svgString], {
      type: "image/svg+xml;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const img = new Image();

    img.onload = () => {
      // Browsers refuse canvases much past 16k px a side.
      const s = Math.min(scale, 12000 / Math.max(img.width, img.height));
      const canvas = document.createElement("canvas");
      canvas.width = Math.round(img.width * s);
      canvas.height = Math.round(img.height * s);
      const ctx = canvas.getContext("2d")!;
      ctx.scale(s, s);
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(img, 0, 0);
      URL.revokeObjectURL(url);
      // Return base64 without the data URI prefix
      resolve(canvas.toDataURL("image/png").split(",")[1]);
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Failed to render SVG to canvas"));
    };

    img.src = url;
  });
}
