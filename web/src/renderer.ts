export interface AgentSnapshot {
  readonly positions: Float32Array;
  readonly tickCount: number;
}

export function drawAgents(
  canvas: HTMLCanvasElement,
  previous: AgentSnapshot,
  current: AgentSnapshot,
  interpolation: number,
): void {
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas 2D context is unavailable");
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#f6efdc";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#702f55";
  for (let index = 0; index < current.positions.length; index += 2) {
    const oldX = previous.positions[index] ?? current.positions[index] ?? 0;
    const oldY = previous.positions[index + 1] ?? current.positions[index + 1] ?? 0;
    const x = oldX + ((current.positions[index] ?? oldX) - oldX) * interpolation;
    const y = oldY + ((current.positions[index + 1] ?? oldY) - oldY) * interpolation;
    context.beginPath();
    context.arc((x / 100) * canvas.width, (y / 100) * canvas.height, 5, 0, Math.PI * 2);
    context.fill();
  }
  canvas.dataset.agentCount = String(current.positions.length / 2);
  canvas.dataset.drawnAgents = String(current.positions.length / 2);
}
