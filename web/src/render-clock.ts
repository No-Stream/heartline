export type RenderFrame<Snapshot> = {
  previous: Snapshot;
  current: Snapshot;
  interpolation: number;
  tick: number;
};

export class RenderClock<Snapshot extends { tickCount: number }> {
  private snapshots: Snapshot[] = [];
  private renderTick: number | undefined;
  private lastFrameTime: number | undefined;

  constructor(private readonly ticksPerSecond: number) {}

  push(snapshot: Snapshot): boolean {
    const latest = this.snapshots.at(-1);
    if (latest && snapshot.tickCount <= latest.tickCount) return false;
    this.snapshots.push(snapshot);
    if (this.renderTick === undefined) this.renderTick = snapshot.tickCount - 3;
    if (this.snapshots.length > 120) {
      this.snapshots.shift();
      this.renderTick = Math.max(this.renderTick, this.snapshots[0].tickCount);
    }
    return true;
  }

  frame(now: number): RenderFrame<Snapshot> | undefined {
    if (this.renderTick === undefined) return undefined;
    const latest = this.snapshots[this.snapshots.length - 1];
    if (this.lastFrameTime !== undefined) {
      const elapsedTicks = Math.max(0, now - this.lastFrameTime) * this.ticksPerSecond / 1000;
      const targetTick = latest.tickCount - 3;
      const correction = Math.max(-0.02, Math.min(0.02, (targetTick - this.renderTick) * 0.01));
      this.renderTick = Math.min(latest.tickCount, this.renderTick + elapsedTicks * (1 + correction));
    }
    this.lastFrameTime = now;
    while (this.snapshots.length > 2 && this.snapshots[1].tickCount <= this.renderTick) this.snapshots.shift();
    const previous = this.snapshots[0];
    const current = this.snapshots[1] ?? previous;
    const tick = Math.max(previous.tickCount, Math.min(current.tickCount, this.renderTick));
    const interpolation = previous === current ? 1 : (tick - previous.tickCount) / (current.tickCount - previous.tickCount);
    return { previous, current, interpolation, tick };
  }
}
