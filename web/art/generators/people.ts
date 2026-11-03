// Public people-generator API. Cast data, classification, and drawing stay in focused modules.
export { CAST, type PersonAppearance, type PersonDesign, type PersonAgeGroup } from "./person-design.ts";
export { personAppearance } from "./person-appearance.ts";
export {
  drawPerson,
  personDocument,
  WALK_FRAME_COUNT,
  IDLE_FRAME_COUNT,
  type Facing,
  type Pose,
} from "./person-drawing.ts";
