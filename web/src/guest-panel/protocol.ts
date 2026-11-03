export type GuestAgeGroup = "child" | "adult" | "senior";

export interface GuestGroupMember {
  readonly id: string;
  readonly name: string;
  readonly relationship: "groupmate" | "parent" | "child" | "partner" | "friend";
}

export interface GuestDestination {
  readonly x: number;
  readonly y: number;
  readonly height: number;
}

export interface GuestActivity {
  readonly kind: string;
  readonly destination: GuestDestination | null;
  readonly placeId: number | null;
}

export interface GuestNeeds {
  readonly hunger: number;
  readonly thirst: number;
  readonly restroom: number;
  readonly nausea: number;
}

export interface GuestTraits {
  readonly intensity: number;
  readonly nauseaTolerance: number;
  readonly favouriteRideTypes: readonly number[];
  readonly themingCare: number;
  readonly preferredTheme: number;
  readonly spending: "budget" | "regular" | "premium";
  readonly heightCm: number;
  readonly walkingSpeed: number;
}

export type GuestMemory =
  | { readonly kind: "ride"; readonly rideId: number; readonly enjoyment: number; readonly nausea: number; readonly tick: number }
  | { readonly kind: "proximity"; readonly rideId: number; readonly eventKind: string; readonly position: { readonly x: number; readonly y: number; readonly z: number }; readonly strength: number; readonly tick: number }
  | { readonly kind: "food"; readonly placeId: number; readonly tick: number }
  | {
    readonly kind: "wait";
    readonly placeId: number;
    readonly durationTicks: number;
    readonly perceivedDurationTicks: number;
    readonly tick: number;
  }
  | { readonly kind: "scenery"; readonly theme: number; readonly enjoyment: number; readonly tick: number };

export type GuestThought =
  | {
    readonly kind: "exploring" | "hungry" | "thirsty" | "needsRestroom" | "nauseous" | "tired" | "unhappy";
    readonly data: null;
  }
  | { readonly kind: "enjoyedRide" | "dislikedRide"; readonly data: number }
  | { readonly kind: "enjoyedScenery"; readonly data: number }
  | { readonly kind: "proximity"; readonly data: { readonly rideId: number; readonly eventKind: string } };

/** Serializable guest state shown by the click-in panel. */
export interface GuestDetail {
  readonly id: string;
  readonly name: string;
  readonly age: GuestAgeGroup;
  readonly groupKind: "single" | "couple" | "family" | "friends";
  readonly groupMembers: readonly GuestGroupMember[];
  readonly activity: GuestActivity;
  readonly needs: GuestNeeds;
  readonly energy: number;
  readonly happiness: number;
  readonly traits: GuestTraits;
  readonly memories: readonly GuestMemory[];
  readonly moneySpent: number;
  readonly lifetimeMoneySpent: number;
  readonly visitNumber: number;
  readonly thought: GuestThought;
  readonly thoughtText: string;
}
