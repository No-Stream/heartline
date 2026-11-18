import { SOUND_TUNING } from "../mapping";

export type BirdStyle = "warble" | "trill" | "whistle" | "chip";
export interface BirdSinger { style: BirdStyle; pitch: number; pan: number; distance: number }
/** Times are seconds from the start of the phrase; `warbleDepth` is in Hz. */
export interface BirdNote { start: number; duration: number; fromFrequency: number; toFrequency: number; warbleRate: number; warbleDepth: number }

const STYLES: readonly BirdStyle[] = ["warble", "trill", "whistle", "chip"];
const PITCH_RANGES: Record<BirdStyle, readonly [number, number]> = { warble: [2200, 3600], trill: [3000, 4800], whistle: [2600, 4000], chip: [3600, 5600] };
const between = (random: () => number, low: number, high: number): number => low + (high - low) * random();

export function createBirdSingers(random: () => number): BirdSinger[] {
  const count = SOUND_TUNING.birds.singers;
  const firstStyle = Math.floor(random() * STYLES.length);
  return Array.from({ length: count }, (_, index) => {
    const style = STYLES[(firstStyle + index) % STYLES.length]!;
    return { style, pitch: between(random, ...PITCH_RANGES[style]), pan: ((index + 0.5) / count * 2 - 1) * 0.8 + between(random, -0.1, 0.1), distance: random() };
  });
}

export function birdGap(random: () => number): number {
  return between(random, ...SOUND_TUNING.birds.gapSeconds);
}

export function birdPhrase(singer: BirdSinger, random: () => number): BirdNote[] {
  const pitch = singer.pitch * between(random, 0.94, 1.06);
  const notes: BirdNote[] = [];
  let time = 0;
  const add = (duration: number, fromFrequency: number, toFrequency: number, warbleRate: number, warbleDepth: number, gap: number): void => {
    notes.push({ start: time, duration, fromFrequency, toFrequency, warbleRate, warbleDepth });
    time += duration + gap;
  };
  switch (singer.style) {
    case "warble": {
      let frequency = pitch;
      for (let index = 3 + Math.floor(random() * 6); index > 0; index--) {
        frequency = Math.min(pitch * 1.4, Math.max(pitch * 0.7, frequency * between(random, 0.8, 1.25)));
        add(between(random, 0.05, 0.12), frequency, frequency * between(random, 0.85, 1.2), between(random, 25, 45), frequency * between(random, 0.02, 0.06), between(random, 0.02, 0.08));
      }
      break;
    }
    case "trill": {
      const count = 8 + Math.floor(random() * 10);
      const drift = between(random, -0.25, 0.2);
      const duration = between(random, 0.025, 0.04);
      const gap = between(random, 0.015, 0.035);
      for (let index = 0; index < count; index++) {
        const frequency = pitch * (1 + drift * index / (count - 1));
        add(duration * between(random, 0.9, 1.1), frequency * 1.08, frequency * 0.9, 0, 0, gap * between(random, 0.85, 1.15));
      }
      break;
    }
    case "whistle": {
      let frequency = pitch;
      for (let index = 2 + Math.floor(random() * 2); index > 0; index--) {
        add(between(random, 0.18, 0.35), frequency, frequency * between(random, 0.96, 1.02), between(random, 6, 10), frequency * 0.01, between(random, 0.08, 0.15));
        frequency *= between(random, 0.8, 0.9);
      }
      break;
    }
    case "chip": {
      for (let index = 0, count = 1 + Math.floor(random() * 3); index < count; index++) {
        const frequency = pitch * (1 - 0.1 * index) * between(random, 0.97, 1.03);
        add(between(random, 0.02, 0.035), frequency * 1.35, frequency * 0.8, 0, 0, between(random, 0.15, 0.4));
      }
      break;
    }
  }
  return notes;
}

/** Plays a phrase on one sine carrier with frequency-modulated warble, so notes glide like a syrinx rather than switching like a beeper. Returns the phrase end time. */
export function playBirdPhrase(context: BaseAudioContext, destination: AudioNode, singer: BirdSinger, notes: readonly BirdNote[], time: number): number {
  const tuning = SOUND_TUNING.birds;
  const level = tuning.level * (1 - (1 - tuning.farLevel) * singer.distance);
  const distanceFilter = context.createBiquadFilter(); distanceFilter.type = "lowpass";
  distanceFilter.frequency.value = tuning.nearCutoff - (tuning.nearCutoff - tuning.farCutoff) * singer.distance;
  const pan = context.createStereoPanner(); pan.pan.value = singer.pan;
  const carrier = context.createOscillator();
  const warble = context.createOscillator();
  const warbleDepth = context.createGain(); warbleDepth.gain.value = 0;
  const envelope = context.createGain(); envelope.gain.value = 0;
  warble.connect(warbleDepth).connect(carrier.frequency);
  carrier.connect(envelope).connect(distanceFilter).connect(pan).connect(destination);
  for (const note of notes) {
    const start = time + note.start;
    const end = start + note.duration;
    carrier.frequency.setValueAtTime(note.fromFrequency, start);
    carrier.frequency.exponentialRampToValueAtTime(note.toFrequency, end);
    warble.frequency.setValueAtTime(Math.max(1, note.warbleRate), start);
    warbleDepth.gain.setValueAtTime(note.warbleDepth, start);
    envelope.gain.setValueAtTime(0, start);
    envelope.gain.linearRampToValueAtTime(level, start + Math.min(0.01, note.duration * 0.3));
    envelope.gain.setTargetAtTime(0, start + note.duration * 0.55, note.duration * 0.12);
  }
  const last = notes.at(-1)!;
  const phraseEnd = time + last.start + last.duration;
  envelope.gain.setValueAtTime(0, phraseEnd + 0.02);
  for (const oscillator of [carrier, warble]) { oscillator.start(time); oscillator.stop(phraseEnd + 0.03); }
  return phraseEnd;
}
