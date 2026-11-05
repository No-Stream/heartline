export const GUEST_PANEL_EDGE = 10;
export const GUEST_PANEL_GAP = 16;

interface Rectangle {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

export function canKeepGuestPanelPosition(canvas: Rectangle, panel: Rectangle, guest: { x: number; y: number }): boolean {
  const insideCanvas = panel.left >= canvas.left + GUEST_PANEL_EDGE
    && panel.right <= canvas.right - GUEST_PANEL_EDGE
    && panel.top >= canvas.top + GUEST_PANEL_EDGE
    && panel.bottom <= canvas.bottom - GUEST_PANEL_EDGE;
  const clearOfGuest = guest.x <= panel.left - GUEST_PANEL_GAP
    || guest.x >= panel.right + GUEST_PANEL_GAP
    || guest.y <= panel.top - GUEST_PANEL_GAP
    || guest.y >= panel.bottom + GUEST_PANEL_GAP;
  return insideCanvas && clearOfGuest;
}
