import { formatClock } from "./render/daylight";
import type { InputAction } from "./input/actions";
import "./print-modern.css";
import "./day-clock.css";

/** How many times faster than the simulation the day can run, to watch a whole day in a minute or two. */
export const DAY_SPEEDS = [1, 10, 60] as const;
export type DaySpeed = (typeof DAY_SPEEDS)[number];
const MINUTES_PER_DAY = 24 * 60;

const sunIcon = "M10 6.5a3.5 3.5 0 1 0 0.01 0M10 1.5v2M10 16.5v2M1.5 10h2M16.5 10h2M4 4l1.4 1.4M14.6 14.6L16 16M4 16l1.4-1.4M14.6 5.4L16 4";
const moonIcon = "M13.5 3.2a7 7 0 1 0 3.3 10.4a5.6 5.6 0 0 1-3.3-10.4z";
const exportIcon = "M10 2.5v10M6 8.5l4 4l4-4M3.5 13.5v3h13v-3";

/**
 * A clock in the game bar with the time of day. While the Watch tool is open it also has a slider to move through the
 * day and a button that changes how fast the day passes.
 */
export function createDayClock(container: HTMLElement, dispatch: (action: InputAction) => void): {
  update(phase: number): void;
  setControlsVisible(visible: boolean): void;
  setDaySpeed(speed: DaySpeed): void;
  cycleGameSpeed(): number;
  setPaused(paused: boolean): void;
} {
  container.classList.add("day-clock");
  container.innerHTML = `
    <svg class="day-clock__icon" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true"><path class="day-clock__icon-plate"/><path class="day-clock__icon-line"/></svg>
    <output class="day-clock__time" aria-label="Time of day" data-day-time></output>
    <span class="day-clock__controls" data-day-controls>
      <input class="day-clock__scrub" type="range" min="0" max="${MINUTES_PER_DAY - 1}" step="1" aria-label="Set the time of day" title="Set the time of day" data-day-scrub>
      <button class="day-clock__speed" type="button" aria-label="Day speed" title="How fast the day passes" data-day-speed>×1</button>
      <button class="day-clock__speed" type="button" aria-label="Game speed" title="Game speed (V)" data-game-speed>×1</button>
      <button class="day-clock__speed day-clock__pause" type="button" aria-label="Pause" title="Pause (Space)" aria-pressed="false" data-game-pause>Ⅱ</button>
      <button class="day-clock__speed" type="button" aria-label="Export session" title="Export session (J)" data-session-export><svg class="day-clock__export-icon" viewBox="0 0 20 20" width="18" height="18" aria-hidden="true"><path d="${exportIcon}"/></svg></button>
    </span>
    <span class="day-clock__paused" data-paused-indicator hidden>Paused</span>`;
  const time = container.querySelector<HTMLOutputElement>("[data-day-time]")!;
  const controls = container.querySelector<HTMLElement>("[data-day-controls]")!;
  const scrub = container.querySelector<HTMLInputElement>("[data-day-scrub]")!;
  const speedButton = container.querySelector<HTMLButtonElement>("[data-day-speed]")!;
  const iconPaths = [...container.querySelectorAll<SVGPathElement>(".day-clock__icon path")];
  const gameSpeedButton = container.querySelector<HTMLButtonElement>("[data-game-speed]")!;
  const pauseButton = container.querySelector<HTMLButtonElement>("[data-game-pause]")!;
  const pausedIndicator = container.querySelector<HTMLElement>("[data-paused-indicator]")!;
  const gameSpeeds = [1, 2, 4, 8, 16];
  let gameSpeedIndex = 0;
  function cycleGameSpeed(): number {
    gameSpeedIndex = (gameSpeedIndex + 1) % gameSpeeds.length;
    gameSpeedButton.textContent = `×${gameSpeeds[gameSpeedIndex]}`;
    return gameSpeeds[gameSpeedIndex]!;
  }
  gameSpeedButton.addEventListener("click", () => dispatch({ type: "game-speed-cycle" }));
  pauseButton.addEventListener("click", () => dispatch({ type: "pause-toggle" }));
  container.querySelector<HTMLButtonElement>("[data-session-export]")!.addEventListener("click", () => dispatch({ type: "session-export" }));
  let speedIndex = 0;
  let shownMinute = -1;
  let dragging = false;
  scrub.addEventListener("pointerdown", () => { dragging = true; });
  scrub.addEventListener("change", () => { dragging = false; });
  scrub.addEventListener("input", () => dispatch({ type: "day-scrub", phase: Number(scrub.value) / MINUTES_PER_DAY }));
  speedButton.addEventListener("click", () => {
    const nextSpeed = DAY_SPEEDS[(speedIndex + 1) % DAY_SPEEDS.length]!;
    dispatch({ type: "day-speed-set", speed: nextSpeed });
  });
  return {
    cycleGameSpeed,
    setPaused(paused) {
      pauseButton.setAttribute("aria-pressed", String(paused));
      pauseButton.setAttribute("aria-label", paused ? "Resume" : "Pause");
      pauseButton.title = paused ? "Resume (Space)" : "Pause (Space)";
      pauseButton.textContent = paused ? "▶" : "Ⅱ";
      container.dataset.paused = String(paused);
      pausedIndicator.hidden = !paused;
    },
    setDaySpeed(speed) {
      speedIndex = DAY_SPEEDS.indexOf(speed);
      if (speedIndex < 0) throw new RangeError(`Unsupported day speed ${speed}`);
      speedButton.textContent = `×${speed}`;
    },
    update(phase) {
      const minute = Math.floor(phase * MINUTES_PER_DAY);
      if (minute === shownMinute) return;
      shownMinute = minute;
      time.value = formatClock(phase);
      const daytime = phase >= 0.25 && phase < 0.75;
      container.dataset.daytime = String(daytime);
      for (const path of iconPaths) path.setAttribute("d", daytime ? sunIcon : moonIcon);
      if (!dragging) scrub.value = String(minute);
    },
    setControlsVisible(visible) { controls.hidden = !visible; },
  };
}
