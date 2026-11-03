import type { Page } from "@playwright/test";

export async function waitForRenderedCameraState(page: Page, rotation: number, zoom: number): Promise<void> {
  await page.waitForFunction(
    ({ expectedRotation, expectedZoom }) => {
      const { renderedRotation, renderedZoom } = document.documentElement.dataset;
      return renderedRotation === String(expectedRotation) && renderedZoom === String(expectedZoom);
    },
    { expectedRotation: rotation, expectedZoom: zoom },
    { polling: "raf" },
  );
}

export async function waitForGalleryRender(page: Page, category: string): Promise<void> {
  await page.waitForFunction(
    (expectedCategory) => {
      const gallery = document.querySelector<HTMLElement>("#gallery");
      const selectedButton = document.querySelector<HTMLButtonElement>(`[data-category="${expectedCategory}"]`);
      if (!gallery || selectedButton?.getAttribute("aria-pressed") !== "true") return false;

      const spriteCanvases = [...gallery.querySelectorAll<HTMLCanvasElement>("canvas.sprite")];
      if (spriteCanvases.length === 0) return false;
      return spriteCanvases.every((canvas) => {
        const context = canvas.getContext("2d");
        if (!context) throw new Error("Gallery sprite canvas has no 2D context");
        const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
        for (let index = 3; index < pixels.length; index += 4) {
          if (pixels[index] !== 0) return true;
        }
        return false;
      });
    },
    category,
    { polling: "raf" },
  );
}
