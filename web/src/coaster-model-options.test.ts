import { describe, expect, it } from "vitest";
import { COASTER_MODELS } from "./generated/coaster-models";
import { coasterModelOptionsMarkup } from "./coaster-model-options";

describe("coaster model select options", () => {
  it("includes every generated coaster model with its stable key and display name", () => {
    const renderedOptions = [...coasterModelOptionsMarkup().matchAll(/<option value="([^"]+)">([^<]+)<\/option>/g)]
      .map(([, value, label]) => ({ value, label }));

    expect(renderedOptions).toEqual(COASTER_MODELS.map(({ key, name, manufacturer }) => ({
      value: `model:${key}`,
      label: `${name} · ${manufacturer}`,
    })));
  });
});
