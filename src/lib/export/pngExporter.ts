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
      const canvas = document.createElement("canvas");
      canvas.width = Math.round(img.width * scale);
      canvas.height = Math.round(img.height * scale);
      const ctx = canvas.getContext("2d")!;
      ctx.scale(scale, scale);
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
