export type RenderFrame<Snapshot> = {
  previous: Snapshot;
  current: Snapshot;
  interpolation: number;
  tick: number;
};

export class RenderClock<Snapshot extends { tickCount: number }> {
  private snapshots: Snapshot[] = [];
  private renderTick: number | undefined;
  private speed = 1;
  private paused = false;
  private bufferingAfterSpeedIncrease = false;
  private lastFrameTime: number | undefined;

  constructor(private readonly ticksPerSecond: number) {}

  setSpeed(speed: number): void {
    if (!Number.isInteger(speed) || speed < 1 || speed > 16) throw new RangeError("Invalid game speed");
    this.bufferingAfterSpeedIncrease = speed > this.speed && this.renderTick !== undefined;
    this.speed = speed;
  }

  setPaused(paused: boolean): void {
    this.paused = paused;
    this.lastFrameTime = undefined;
  }

  snap(snapshot: Snapshot): void {
    this.snapshots = [snapshot];
    this.renderTick = snapshot.tickCount;
    this.bufferingAfterSpeedIncrease = false;
    this.lastFrameTime = undefined;
  }

  push(snapshot: Snapshot): boolean {
    const latest = this.snapshots.at(-1);
    if (latest && snapshot.tickCount <= latest.tickCount) return false;
    this.snapshots.push(snapshot);
    if (this.renderTick === undefined) this.renderTick = snapshot.tickCount - 3 * this.speed;
    if (this.snapshots.length > 120) {
      this.snapshots.shift();
      this.renderTick = Math.max(this.renderTick, this.snapshots[0].tickCount);
    }
    return true;
  }

  frame(now: number): RenderFrame<Snapshot> | undefined {
    if (this.renderTick === undefined) return undefined;
    const latest = this.snapshots[this.snapshots.length - 1];
    if (!this.paused && this.lastFrameTime !== undefined) {
      const elapsedTicks = Math.max(0, now - this.lastFrameTime) * this.ticksPerSecond * this.speed / 1000;
      const targetTick = latest.tickCount - 3 * this.speed;
      if (this.bufferingAfterSpeedIncrease && targetTick >= this.renderTick) this.bufferingAfterSpeedIncrease = false;
      const correction = Math.max(-0.02, Math.min(0.02, (targetTick - this.renderTick) * 0.01));
      if (!this.bufferingAfterSpeedIncrease) this.renderTick = Math.min(latest.tickCount, this.renderTick + elapsedTicks * (1 + correction));
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
