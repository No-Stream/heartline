import { COASTER_MODELS } from "./generated/coaster-models";

/** The native select's options come directly from the simulation's coaster catalog. */
export function coasterModelOptionsMarkup(): string {
  return COASTER_MODELS.map(({ key, name, manufacturer }) =>
    `<option value="model:${key}">${name} · ${manufacturer}</option>`).join("");
}
