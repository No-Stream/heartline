import { COASTER_MODELS } from "./generated/coaster-models";
import { DEMO_COASTER_ID, type CoasterListEntry } from "./protocol";
import type { TrainLayout } from "./train-layout";

interface RenderedCoasterListEntry {
  readonly id: number;
  readonly trainLayout: Pick<TrainLayout, "model"> | null;
}

export function coasterListEntries(
  coasters: readonly CoasterListEntry[],
  renderedCoasters: readonly RenderedCoasterListEntry[],
): CoasterListEntry[] {
  if (coasters.some(({ id }) => id === DEMO_COASTER_ID)) return [...coasters];
  const demoCoaster = renderedCoasters.find(({ id }) => id === DEMO_COASTER_ID);
  if (!demoCoaster) return [...coasters];

  const modelKey = demoCoaster.trainLayout?.model;
  if (!modelKey) throw new Error("Demo coaster render data is missing its model");
  const model = COASTER_MODELS.find(({ key }) => key === modelKey);
  if (!model) throw new Error(`Unknown demo coaster model ${modelKey}`);

  return [...coasters, {
    id: DEMO_COASTER_ID,
    name: model.name,
    model: model.key,
    open: false,
    price: 0,
    income: 0,
    queueLength: 0,
    complete: true,
  }];
}
