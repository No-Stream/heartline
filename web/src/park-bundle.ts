interface ParkDescriptionSource {
  readonly coasters: readonly (Readonly<Record<string, unknown>> & { readonly ride: string })[];
}

type RideDescriptions = Readonly<Record<string, Readonly<Record<string, unknown>>>>;

const parkDescriptions = import.meta.glob<ParkDescriptionSource>("../../content/parks/*.park.json", {
  eager: true,
  import: "default",
});
const rideDescriptions = import.meta.glob<Readonly<Record<string, unknown>>>("../../content/rides/*.ride.json", {
  eager: true,
  import: "default",
});

export function bundledParkDescriptionJson(name: string): string {
  const parkPath = `../../content/parks/${name}.park.json`;
  const parkDescription = parkDescriptions[parkPath];
  if (!parkDescription) throw new Error(`Unknown park description: ${name}`);

  return bundleParkDescriptionJson(name, parkDescription, rideDescriptions);
}

export function bundleParkDescriptionJson(
  name: string,
  parkDescription: ParkDescriptionSource,
  availableRides: RideDescriptions,
): string {
  if (parkDescription.coasters.length > 1) {
    throw new Error("this build supports one player coaster per park");
  }

  const coasters = parkDescription.coasters.map((coaster, index) => {
    const ridePath = coaster.ride;
    if (!ridePath.startsWith("content/rides/")) {
      throw new Error(`Park ${name} coaster ${index} ride must be under content/rides/: ${ridePath}`);
    }
    const rideModulePath = `../../${ridePath}`;
    const rideDescription = availableRides[rideModulePath];
    if (!rideDescription) {
      throw new Error(`Park ${name} coaster ${index} references missing ride file: ${ridePath}`);
    }
    return { ...coaster, ride: rideDescription };
  });

  return JSON.stringify({ ...parkDescription, coasters });
}
