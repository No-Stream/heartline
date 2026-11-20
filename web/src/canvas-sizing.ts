export interface CanvasBackingSize {
  readonly width: number;
  readonly height: number;
  readonly pixelRatio: number;
}

export function canvasBackingSizeForBounds(
  bounds: { readonly width: number; readonly height: number },
  devicePixelRatio: number,
): CanvasBackingSize | null {
  if (bounds.width <= 0 || bounds.height <= 0) return null;

  const width = Math.max(1, Math.round(bounds.width * devicePixelRatio));
  const height = Math.max(1, Math.round(bounds.height * devicePixelRatio));
  return {
    width,
    height,
    pixelRatio: width / bounds.width,
  };
}
