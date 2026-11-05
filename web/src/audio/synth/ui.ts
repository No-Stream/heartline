import type { TrackMaterial, UiEvent } from "../mapping";
import { noiseBuffers } from "./noise";

export interface UiSoundHandle { readonly end: number; stop(atTime: number): void }

/**
 * Build and interface sounds. Levels are linear gains before the master gain (volume²) and the compressor.
 * Times are seconds; `decay` is the time constant of an exponential fall, so a part is near silent after about six of them.
 */
export const UI_TUNING = {
  /** Each placement scales pitch and level by up to these fractions, so a run of placements does not repeat exactly. */
  variation: { pitch: 0.05, level: 0.1 },
  place: {
    body: { from: 120, to: 52, glide: 0.09, level: 0.55, decay: { steel: 0.06, wood: 0.045 } },
    knock: { lowpass: { steel: 1100, wood: 700 }, level: 0.45, decay: 0.012 },
    steelClick: { bandpass: 3400, q: 2.5, level: 0.5, decay: 0.006 },
    steelRing: { frequency: 1870, level: 0.025, decay: 0.07 },
    woodClick: { bandpass: 1800, q: 1.5, level: 0.35, decay: 0.008 },
    woodTok: { from: 300, to: 240, level: 0.15, decay: 0.025 },
  },
  build: { body: { from: 170, to: 95, glide: 0.06, level: 0.3, decay: 0.035 }, knock: { lowpass: 1400, level: 0.2, decay: 0.008 } },
  undo: { body: { from: 95, to: 170, glide: 0.07, level: 0.28, decay: 0.04 }, whoosh: { from: 500, to: 1600, sweep: 0.08, q: 1.2, level: 0.25, decay: 0.03 } },
  error: { notes: [{ frequency: 220, length: 0.075, level: 0.2 }, { frequency: 175, length: 0.12, level: 0.17 }], gap: 0.03, lowpass: 700, release: 0.025 },
  finish: { notes: [523.25, 659.25, 783.99, 1046.5], spacing: 0.075, level: 0.11, overtone: 0.015, decay: 0.16, lastDecay: 0.45, sparkle: { highpass: 5000, level: 0.02, decay: 0.25 } },
};

interface Part { readonly start: number; readonly level: number; readonly decay: number; readonly attack?: number }

/** One envelope: silent until `start`, a short linear attack to `level`, then an exponential fall. Returns when it is near silent. */
function envelope(context: BaseAudioContext, destination: AudioNode, part: Part): { gain: GainNode; end: number } {
  const attack = part.attack ?? 0.002;
  const gain = context.createGain();
  gain.gain.setValueAtTime(0, part.start);
  gain.gain.linearRampToValueAtTime(part.level, part.start + attack);
  gain.gain.setTargetAtTime(0, part.start + attack, part.decay);
  gain.connect(destination);
  return { gain, end: part.start + attack + 7 * part.decay };
}

function tone(context: BaseAudioContext, destination: AudioNode, part: Part & { type: OscillatorType; from: number; to: number; glide: number }): number {
  const { gain, end } = envelope(context, destination, part);
  const oscillator = context.createOscillator();
  oscillator.type = part.type;
  oscillator.frequency.setValueAtTime(part.from, part.start);
  oscillator.frequency.exponentialRampToValueAtTime(part.to, part.start + part.glide);
  oscillator.connect(gain);
  oscillator.start(part.start);
  oscillator.stop(end);
  return end;
}

/** A burst of the shared white noise through one filter, starting at a random point in the buffer. */
function noise(context: BaseAudioContext, destination: AudioNode, part: Part & { filter: BiquadFilterType; frequency: number; q: number; sweepTo?: number; sweep?: number; offset: number }): number {
  const { gain, end } = envelope(context, destination, part);
  const filter = context.createBiquadFilter();
  filter.type = part.filter;
  filter.frequency.setValueAtTime(part.frequency, part.start);
  if (part.sweepTo !== undefined && part.sweep !== undefined) filter.frequency.exponentialRampToValueAtTime(part.sweepTo, part.start + part.sweep);
  filter.Q.value = part.q;
  const source = context.createBufferSource();
  source.buffer = noiseBuffers(context).white;
  source.connect(filter).connect(gain);
  source.start(part.start, part.offset, end - part.start);
  return end;
}

function placeTrack(context: BaseAudioContext, out: AudioNode, at: number, material: TrackMaterial, random: () => number): number {
  const tuning = UI_TUNING.place;
  const pitch = 1 + UI_TUNING.variation.pitch * (2 * random() - 1);
  const level = 1 - UI_TUNING.variation.level * random();
  const offset = random();
  const timber = material === "wood";
  const ends = [
    tone(context, out, { type: "sine", start: at, from: tuning.body.from * pitch, to: tuning.body.to * pitch, glide: tuning.body.glide, level: tuning.body.level * level, decay: timber ? tuning.body.decay.wood : tuning.body.decay.steel, attack: 0.003 }),
    noise(context, out, { filter: "lowpass", frequency: timber ? tuning.knock.lowpass.wood : tuning.knock.lowpass.steel, q: 1, start: at, level: tuning.knock.level * level, decay: tuning.knock.decay, attack: 0.001, offset }),
  ];
  const steelShare = { steel: 1, hybrid: 0.5, wood: 0 }[material];
  const woodShare = { steel: 0, hybrid: 0.5, wood: 1 }[material];
  if (steelShare > 0) {
    ends.push(noise(context, out, { filter: "bandpass", frequency: tuning.steelClick.bandpass * pitch, q: tuning.steelClick.q, start: at, level: tuning.steelClick.level * level, decay: tuning.steelClick.decay, attack: 0.0005, offset: offset + 0.5 }));
    const ring = tuning.steelRing.frequency * pitch;
    ends.push(tone(context, out, { type: "sine", start: at, from: ring, to: ring, glide: 0.01, level: tuning.steelRing.level * steelShare * level, decay: tuning.steelRing.decay }));
  }
  if (woodShare > 0) {
    if (steelShare === 0) ends.push(noise(context, out, { filter: "bandpass", frequency: tuning.woodClick.bandpass * pitch, q: tuning.woodClick.q, start: at, level: tuning.woodClick.level * level, decay: tuning.woodClick.decay, attack: 0.0005, offset: offset + 0.5 }));
    ends.push(tone(context, out, { type: "triangle", start: at, from: tuning.woodTok.from * pitch, to: tuning.woodTok.to * pitch, glide: 0.03, level: tuning.woodTok.level * woodShare * level, decay: tuning.woodTok.decay }));
  }
  return Math.max(...ends);
}

function build(context: BaseAudioContext, out: AudioNode, at: number, random: () => number): number {
  const { body, knock } = UI_TUNING.build;
  const pitch = 1 + UI_TUNING.variation.pitch * (2 * random() - 1);
  return Math.max(
    tone(context, out, { type: "sine", start: at, from: body.from * pitch, to: body.to * pitch, glide: body.glide, level: body.level, decay: body.decay, attack: 0.003 }),
    noise(context, out, { filter: "lowpass", frequency: knock.lowpass, q: 0.8, start: at, level: knock.level, decay: knock.decay, attack: 0.001, offset: random() }),
  );
}

function undo(context: BaseAudioContext, out: AudioNode, at: number, random: () => number): number {
  const { body, whoosh } = UI_TUNING.undo;
  return Math.max(
    tone(context, out, { type: "sine", start: at, from: body.from, to: body.to, glide: body.glide, level: body.level, decay: body.decay, attack: 0.004 }),
    noise(context, out, { filter: "bandpass", frequency: whoosh.from, sweepTo: whoosh.to, sweep: whoosh.sweep, q: whoosh.q, start: at, level: whoosh.level, decay: whoosh.decay, attack: 0.01, offset: random() }),
  );
}

/** Two soft falling notes, low-passed so nearly all their energy sits below 1 kHz. Each note droops in pitch and level while it holds. */
function error(context: BaseAudioContext, out: AudioNode, at: number): number {
  const tuning = UI_TUNING.error;
  const filter = context.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = tuning.lowpass;
  filter.Q.value = 0.7;
  filter.connect(out);
  let start = at;
  let end = at;
  for (const note of tuning.notes) {
    const gain = context.createGain();
    gain.gain.setValueAtTime(0, start);
    gain.gain.linearRampToValueAtTime(note.level, start + 0.006);
    gain.gain.setTargetAtTime(note.level * 0.6, start + 0.006, 0.05);
    gain.gain.setTargetAtTime(0, start + note.length, tuning.release);
    gain.connect(filter);
    end = start + note.length + 7 * tuning.release;
    for (const [type, frequency, share] of [["triangle", note.frequency, 1], ["sine", note.frequency / 2, 0.5]] as const) {
      const oscillator = context.createOscillator();
      const voice = context.createGain();
      voice.gain.value = share;
      oscillator.type = type;
      oscillator.frequency.setValueAtTime(frequency, start);
      oscillator.frequency.exponentialRampToValueAtTime(frequency * 0.97, start + note.length);
      oscillator.connect(voice).connect(gain);
      oscillator.start(start);
      oscillator.stop(end);
    }
    start += note.length + tuning.gap;
  }
  return end;
}

/** A rising C major arpeggio with a faint overtone, over the soft build thunk, ending in a quiet high sparkle. */
function finish(context: BaseAudioContext, out: AudioNode, at: number, random: () => number): number {
  const tuning = UI_TUNING.finish;
  const ends = [build(context, out, at, random)];
  tuning.notes.forEach((frequency, index) => {
    const start = at + index * tuning.spacing;
    const decay = index === tuning.notes.length - 1 ? tuning.lastDecay : tuning.decay;
    ends.push(tone(context, out, { type: "sine", start, from: frequency, to: frequency, glide: 0.01, level: tuning.level, decay, attack: 0.004 }));
    ends.push(tone(context, out, { type: "sine", start, from: frequency * 3, to: frequency * 3, glide: 0.01, level: tuning.overtone, decay: decay / 2, attack: 0.004 }));
  });
  const last = at + (tuning.notes.length - 1) * tuning.spacing;
  ends.push(noise(context, out, { filter: "highpass", frequency: tuning.sparkle.highpass, q: 0.7, start: last, level: tuning.sparkle.level, decay: tuning.sparkle.decay, attack: 0.12, offset: random() }));
  return Math.max(...ends);
}

/** Plays one interface sound into `destination` at `atTime`. `random` returns values in [0, 1) and drives the small per-play variation. */
export function playUiSound(context: BaseAudioContext, destination: AudioNode, event: UiEvent, atTime: number, random: () => number): UiSoundHandle {
  const output = context.createGain();
  output.connect(destination);
  const end = event.sound === "place" ? placeTrack(context, output, atTime, event.material, random)
    : event.sound === "build" ? build(context, output, atTime, random)
      : event.sound === "undo" ? undo(context, output, atTime, random)
        : event.sound === "error" ? error(context, output, atTime)
          : finish(context, output, atTime, random);
  return {
    end,
    stop(stopTime: number): void {
      output.gain.cancelScheduledValues(stopTime);
      output.gain.setTargetAtTime(0, stopTime, 0.005);
    },
  };
}
