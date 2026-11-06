import type { ConstructionForceLimitsWire } from "../generated/ConstructionForceLimitsWire";
import type { ConstructionForceRatingBandWire } from "../generated/ConstructionForceRatingBandWire";

interface ForceLimitsFixtureInput {
  readonly minimumVerticalG: number;
  readonly maximumVerticalG: number;
  readonly greatPositiveVerticalG: number;
  readonly maximumLateralG: number;
  readonly maximumLongitudinalG: number;
}

function ratingBand(
  rating: ConstructionForceRatingBandWire["rating"],
  upTo: number | null,
  inclusive: boolean,
): ConstructionForceRatingBandWire {
  return { rating, upTo, inclusive };
}

export function forceLimitsFixture(input: ForceLimitsFixtureInput): ConstructionForceLimitsWire {
  const { minimumVerticalG, maximumVerticalG, greatPositiveVerticalG, maximumLateralG, maximumLongitudinalG } = input;
  const dangerous = (): ConstructionForceRatingBandWire => ratingBand("dangerous", null, true);

  return {
    minimumVerticalG,
    maximumVerticalG,
    maximumLateralG,
    maximumLongitudinalG,
    ratingBands: {
      airtime: [
        ratingBand("weak", 0.2, true),
        ratingBand("good", 0.9, true),
        ratingBand("great", -minimumVerticalG, true),
        ratingBand("too much", -minimumVerticalG * 1.3, true),
        dangerous(),
      ],
      vertical: [
        ratingBand("weak", 2, true),
        ratingBand("good", greatPositiveVerticalG, false),
        ratingBand("great", maximumVerticalG, true),
        ratingBand("too much", maximumVerticalG * 1.3, true),
        dangerous(),
      ],
      lateral: [
        ratingBand("fine", maximumLateralG * 0.5, true),
        ratingBand("strong", maximumLateralG, true),
        ratingBand("too much", maximumLateralG * 1.3, true),
        dangerous(),
      ],
      longitudinal: [
        ratingBand("fine", maximumLongitudinalG * 0.5, true),
        ratingBand("strong", maximumLongitudinalG, true),
        ratingBand("too much", maximumLongitudinalG * 1.3, true),
        dangerous(),
      ],
    },
  };
}
