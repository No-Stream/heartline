import { displaySpeedToMetresPerSecond, metresPerSecondToDisplay, type Units } from "./units";

type LaunchSpeedInput = Pick<HTMLInputElement, "min" | "max" | "step" | "value" | "valueAsNumber" | "dataset" | "setAttribute" | "addEventListener">;
type LaunchSpeedAction = { readonly type: "construction-set-launch-intensity"; readonly id: number; readonly targetSpeed: number };

interface LaunchSpeedInputOptions {
  readonly id: number;
  readonly targetSpeed: number;
  readonly minimumSpeed: number;
  readonly maximumSpeed: number;
  readonly units: Units;
  readonly label: string;
  readonly dispatch: (action: LaunchSpeedAction) => void;
}

export function configureLaunchSpeedInput(input: LaunchSpeedInput, options: LaunchSpeedInputOptions): void {
  const minimumDisplaySpeed = metresPerSecondToDisplay(options.minimumSpeed, options.units);
  const maximumDisplaySpeed = metresPerSecondToDisplay(options.maximumSpeed, options.units);
  input.min = minimumDisplaySpeed.toFixed(1);
  input.max = maximumDisplaySpeed.toFixed(1);
  input.step = "0.1";
  input.value = metresPerSecondToDisplay(options.targetSpeed, options.units).toFixed(1);
  input.dataset.minimumDisplaySpeed = String(minimumDisplaySpeed);
  input.dataset.maximumDisplaySpeed = String(maximumDisplaySpeed);
  input.setAttribute("aria-label", options.label);
  input.setAttribute("data-launch-target-speed", String(options.targetSpeed));
  input.addEventListener("change", () => {
    if (!Number.isFinite(input.valueAsNumber)) return;
    const speed = Math.max(minimumDisplaySpeed, Math.min(maximumDisplaySpeed, input.valueAsNumber));
    options.dispatch({ type: "construction-set-launch-intensity", id: options.id, targetSpeed: displaySpeedToMetresPerSecond(speed, options.units) });
  });
}
