import type { GeometryQuad, Triangle, Vertex } from "./scene";

const GROUND_KINDS: ReadonlySet<string> = new Set(["terrain", "cliff", "water", "path", "carved-floor", "cavern-wall"]);

function edgeKey(first: Vertex, second: Vertex): string {
  const point = (vertex: Vertex): string => `${vertex.x.toFixed(2)},${vertex.y.toFixed(2)}`;
  return [point(first), point(second)].sort().join("|");
}

/** Screen-space tile borders, deduplicated so neighboring tiles do not darken their shared edge. */
export function buildGridQuads(triangles: readonly Triangle[], color: readonly [number, number, number, number], width = 0.8): GeometryQuad[] {
  const quads: GeometryQuad[] = [];
  const edges = new Set<string>();
  for (let index = 0; index + 1 < triangles.length; index += 2) {
    const first = triangles[index];
    const second = triangles[index + 1];
    if (!GROUND_KINDS.has(first.kind) || first.tileX !== second.tileX || first.tileY !== second.tileY) continue;
    const corners = [first.vertices[0], first.vertices[1], first.vertices[2], second.vertices[2]];
    for (let corner = 0; corner < corners.length; corner++) {
      const start = corners[corner];
      const end = corners[(corner + 1) % corners.length];
      const key = edgeKey(start, end);
      if (edges.has(key)) continue;
      edges.add(key);
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const length = Math.hypot(dx, dy);
      if (length === 0) continue;
      const offsetX = -dy / length * width / 2;
      const offsetY = dx / length * width / 2;
      const vertex = (point: Vertex, across: number, down: number): Vertex => ({
        ...point,
        x: point.x + across,
        y: point.y + down,
        u: across === 0 ? 0 : 1,
        v: down === 0 ? 0 : 1,
      });
      quads.push({
        vertices: [vertex(start, offsetX, offsetY), vertex(end, offsetX, offsetY), vertex(end, -offsetX, -offsetY), vertex(start, -offsetX, -offsetY)],
        color,
        kind: "grid",
        id: `grid-${first.tileX}-${first.tileY}-${corner}`,
        tileX: first.tileX,
        tileY: first.tileY,
      });
    }
  }
  return quads;
}
