import { describe, expect, it } from "vitest";
import { configureLaunchSpeedInput } from "./construction-launch-speed";

describe("launch speed input", () => {
  it("shows one decimal place, keeps exact bounds, and dispatches the entered speed", () => {
    let onChange: (() => void) | undefined;
    const attributes = new Map<string, string>();
    const input = {
      min: "",
      max: "",
      step: "",
      value: "",
      valueAsNumber: Number.NaN,
      dataset: {} as DOMStringMap,
      setAttribute: (name: string, value: string) => attributes.set(name, value),
      addEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => {
        onChange = listener as () => void;
      },
    };
    const actions: { readonly type: "construction-set-launch-intensity"; readonly id: number; readonly targetSpeed: number }[] = [];

    configureLaunchSpeedInput(input, {
      id: 7,
      targetSpeed: 14,
      minimumSpeed: 13.125,
      maximumSpeed: 22.5,
      units: "metric",
      label: "Launch 2 target (km/h)",
      dispatch: (action) => actions.push(action),
    });

    expect(input).toMatchObject({ min: "47.3", max: "81.0", step: "0.1", value: "50.4" });
    expect(input.dataset).toEqual({ minimumDisplaySpeed: "47.25", maximumDisplaySpeed: "81" });
    expect(attributes.get("data-launch-target-speed")).toBe("14");
    input.valueAsNumber = 53.2;
    onChange?.();
    input.valueAsNumber = 81.1;
    onChange?.();
    expect(actions).toEqual([
      { type: "construction-set-launch-intensity", id: 7, targetSpeed: 53.2 / 3.6 },
      { type: "construction-set-launch-intensity", id: 7, targetSpeed: 22.5 },
    ]);
  });
});
