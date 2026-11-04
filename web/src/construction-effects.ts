import { projectWorld, type Camera, type Viewport, type WorldPoint } from "./camera";
import { railPoints } from "./construction-camera";
import type { ConstructionState } from "./construction-protocol";
import type { GeometryQuad, Vertex } from "./render/scene";
import { buildTrackQuads, type TrackStyle } from "./render/track/geometry";

const snapDuration = 420;
const celebrationDuration = 2200;
const confettiInks = ["#227571", "#dc5c3e", "#e9ab38", "#3f61a6", "#f2eadb"];
const noGeometry = new Float32Array();

type Effect =
  | { readonly kind: "snap"; readonly start: number; readonly rails: Float32Array; readonly ties: Float32Array; readonly joint: WorldPoint }
  | { readonly kind: "celebration"; readonly start: number; readonly rails: Float32Array; readonly confetti: readonly Confetti[] };
interface Confetti { readonly origin: WorldPoint; readonly velocity: WorldPoint; readonly spin: number; readonly color: readonly [number, number, number, number] }

const rgba = (hex: string, alpha = 1): [number, number, number, number] => [parseInt(hex.slice(1, 3), 16) / 255, parseInt(hex.slice(3, 5), 16) / 255, parseInt(hex.slice(5, 7), 16) / 255, alpha];
/** Lift a highlight just in front of the track it covers so it is not hidden by the depth test. */
const raised = (quad: GeometryQuad): GeometryQuad => ({ ...quad, vertices: quad.vertices.map((vertex) => ({ ...vertex, depth: vertex.depth + 0.2 })) as GeometryQuad["vertices"] });
const depthOf = (point: WorldPoint, rotation: Camera["rotation"]): number => [point.x + point.y, point.x - point.y, -point.x - point.y, -point.x + point.y][rotation] + point.z;

/** Recognises a placed piece and a closed circuit from consecutive builder states. */
export function constructionEvent(previous: ConstructionState | undefined, next: ConstructionState): "placed" | "closed" | undefined {
  if (!previous?.active || !next.active) return undefined;
  if (previous.finishPreview && !next.finishPreview && next.canTest) return "closed";
  if (next.track.length > previous.track.length && previous.previewRails.length > 0) return "placed";
  if (previous.canClose && !next.canClose && next.track.length === previous.track.length) return "closed";
  return undefined;
}

/** A short ink burst where a piece clicks into place, and confetti with a glint around the circuit when it closes. */
export function createConstructionEffects(style: TrackStyle) {
  let effects: Effect[] = [];
  let seed = 1;
  const random = (): number => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  const flat = (points: readonly Vertex[]): GeometryQuad["vertices"] => points as GeometryQuad["vertices"];

  function snapQuads(effect: Extract<Effect, { kind: "snap" }>, progress: number, camera: Camera, viewport: Viewport): GeometryQuad[] {
    const fade = 1 - progress;
    const ink = rgba(style.ink);
    const glow = 0.95 * Math.min(1, fade / 0.6);
    const highlight = buildTrackQuads(effect.rails, effect.ties, noGeometry, noGeometry, camera, viewport, style)
      .filter((quad) => quad.color.some((channel, index) => channel !== ink[index]))
      .map((quad) => raised({ ...quad, id: `effect-${quad.id}`, kind: "effect", color: rgba("#f6d77a", glow) }));
    const centre = projectWorld(camera, viewport, effect.joint);
    const depth = depthOf(effect.joint, camera.rotation) + 2;
    const reach = 1 - (1 - progress) ** 3;
    const inner = (6 + 10 * reach) * camera.zoom, outer = (11 + 16 * reach) * camera.zoom, width = 1.6 * camera.zoom;
    const ticks = Array.from({ length: 8 }, (_, index): GeometryQuad => {
      const angle = index * Math.PI / 4 + Math.PI / 8;
      const [cos, sin] = [Math.cos(angle), Math.sin(angle) * 0.6];
      const [normalX, normalY] = [-sin, cos];
      const at = (radius: number, side: number): Vertex => ({ x: centre.x + cos * radius + normalX * side, y: centre.y + sin * radius + normalY * side, depth, height: effect.joint.z * 4, u: 0, v: 0 });
      return { vertices: flat([at(inner, -width), at(outer, -width), at(outer, width), at(inner, width)]), color: rgba(index % 2 ? "#f2eadb" : "#e9ab38", fade), kind: "effect", id: `effect-snap-${index}`, tileX: Math.floor(effect.joint.x), tileY: Math.floor(effect.joint.y) };
    });
    return [...highlight, ...ticks];
  }

  function celebrationQuads(effect: Extract<Effect, { kind: "celebration" }>, elapsed: number, camera: Camera, viewport: Viewport): GeometryQuad[] {
    const progress = elapsed / celebrationDuration;
    const segments = buildTrackQuads(effect.rails, noGeometry, noGeometry, noGeometry, camera, viewport, style);
    const glint = Math.min(1, elapsed / 1300);
    const lit = segments.filter((_, index) => Math.abs(index / segments.length - glint) < 0.05 && glint < 1)
      .map((quad) => raised({ ...quad, id: `effect-${quad.id}`, kind: "effect", color: rgba("#f4cf6a", 0.95) }));
    const seconds = elapsed / 1000;
    const fade = Math.min(1, (1 - progress) * 3);
    const confetti = effect.confetti.map((piece, index): GeometryQuad => {
      const drag = (1 - Math.exp(-1.6 * seconds)) / 1.6;
      const position = { x: piece.origin.x + piece.velocity.x * drag, y: piece.origin.y + piece.velocity.y * drag, z: piece.origin.z + piece.velocity.z * drag - 0.9 * seconds * seconds };
      const centre = projectWorld(camera, viewport, position);
      const flutter = Math.cos(piece.spin * seconds * 6);
      const angle = piece.spin * seconds * 3;
      const [half, thin] = [3.6 * camera.zoom, 2.2 * camera.zoom * Math.max(0.25, Math.abs(flutter))];
      const corner = (along: number, across: number): Vertex => ({ x: centre.x + Math.cos(angle) * along - Math.sin(angle) * across, y: centre.y + Math.sin(angle) * along + Math.cos(angle) * across, depth: depthOf(position, camera.rotation) + 3, height: position.z * 4, u: 0, v: 0 });
      return { vertices: flat([corner(-half, -thin), corner(half, -thin), corner(half, thin), corner(-half, thin)]), color: [piece.color[0], piece.color[1], piece.color[2], fade], kind: "effect", id: `effect-confetti-${index}`, tileX: Math.floor(position.x), tileY: Math.floor(position.y) };
    });
    return [...lit, ...confetti];
  }

  return {
    observe(previous: ConstructionState | undefined, next: ConstructionState, now: number): void {
      const event = constructionEvent(previous, next);
      if (event === "placed") {
        const points = railPoints(previous!.previewRails);
        effects.push({ kind: "snap", start: now, rails: previous!.previewRails, ties: previous!.previewTies, joint: points[points.length - 1] });
      } else if (event === "closed") {
        const station = railPoints(next.placedRails)[0];
        const confetti = Array.from({ length: 90 }, (): Confetti => {
          const heading = random() * Math.PI * 2;
          const speed = 0.6 + random() * 1.6;
          return { origin: { x: station.x + (random() - 0.5) * 2, y: station.y + (random() - 0.5) * 2, z: station.z + 0.6 }, velocity: { x: Math.cos(heading) * speed, y: Math.sin(heading) * speed, z: 2.2 + random() * 2.6 }, spin: (random() - 0.5) * 4, color: rgba(confettiInks[Math.floor(random() * confettiInks.length)]) };
        });
        effects.push({ kind: "celebration", start: now, rails: next.placedRails, confetti });
      }
    },
    active(now: number): boolean {
      effects = effects.filter((effect) => now - effect.start < (effect.kind === "snap" ? snapDuration : celebrationDuration));
      return effects.length > 0;
    },
    quads(now: number, camera: Camera, viewport: Viewport): GeometryQuad[] {
      return effects.flatMap((effect) => {
        const elapsed = Math.max(0, now - effect.start);
        return effect.kind === "snap" ? snapQuads(effect, Math.min(1, elapsed / snapDuration), camera, viewport) : celebrationQuads(effect, elapsed, camera, viewport);
      });
    },
  };
}
