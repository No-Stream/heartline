export const MAX_TRAIN_VOICES = 4;
export const MAX_SCREAM_GROUPS = 3;
export const MAX_UI_VOICES = 4;
const MIN_AUDIBILITY = 0.02;

export function allocateTrainVoices(previous: readonly (number | null)[], trains: readonly { train: number; audibility: number }[]): (number | null)[] {
  const eligible = trains.filter((train) => train.audibility >= MIN_AUDIBILITY);
  const byId = new Map(eligible.map((train) => [train.train, train]));
  const voices = Array.from({ length: MAX_TRAIN_VOICES }, (_, index) => {
    const train = previous[index];
    return train !== undefined && train !== null && byId.has(train) ? train : null;
  });
  const challengers = eligible.filter((train) => !voices.includes(train.train)).sort((a, b) => b.audibility - a.audibility);
  for (const challenger of challengers) {
    const free = voices.indexOf(null);
    if (free >= 0) { voices[free] = challenger.train; continue; }
    let quietest = 0;
    for (let index = 1; index < voices.length; index++) {
      if (byId.get(voices[index]!)!.audibility < byId.get(voices[quietest]!)!.audibility) quietest = index;
    }
    if (challenger.audibility > byId.get(voices[quietest]!)!.audibility * 1.1) voices[quietest] = challenger.train;
  }
  return voices;
}
