import { useCallback, useEffect, useState } from "react";

/**
 * An element's width, kept current as it resizes. Returns a callback ref, so
 * it follows the element even when it mounts after the first render.
 */
export function useWidth<T extends HTMLElement>(fallback = 600): [(el: T | null) => void, number] {
  const [el, setEl] = useState<T | null>(null);
  const [width, setWidth] = useState(fallback);
  useEffect(() => {
    if (!el) return;
    const measure = () => setWidth(el.clientWidth || fallback);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [el, fallback]);
  return [useCallback((node: T | null) => setEl(node), []), width];
}
