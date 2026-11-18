import { SOUND_TUNING } from "./mapping";
import amusementParkChildrenSwell from "./recordings/screams/amusement-park-children-swell.mp3";
import coasterGirlsFirstBurst from "./recordings/screams/coaster-girls-first-burst.mp3";
import coasterGirlsSecondBurst from "./recordings/screams/coaster-girls-second-burst.mp3";
import amusementParkGirlsFirstBurst from "./recordings/screams/amusement-park-girls-first-burst.mp3";
import amusementParkGirlsSecondBurst from "./recordings/screams/amusement-park-girls-second-burst.mp3";

/** CC0 recordings; sources and licences are in recordings/CREDITS.md. */
export const SCREAM_RECORDING_URLS: readonly string[] = [amusementParkChildrenSwell, coasterGirlsFirstBurst, coasterGirlsSecondBurst, amusementParkGirlsFirstBurst, amusementParkGirlsSecondBurst];

/** `normalization` scales the clip to unit RMS so SOUND_TUNING.scream.level sets the loudness of every variant. */
export interface ScreamRecording { buffer: AudioBuffer; normalization: number }

export function longestScreamSeconds(): number {
  const tuning = SOUND_TUNING.scream;
  return tuning.attack + tuning.hold[0] + tuning.hold[1] + tuning.release;
}

export interface SoundRecordings { screams: ScreamRecording[] }

async function decodeMono(context: BaseAudioContext, url: string): Promise<AudioBuffer> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Recording ${url} failed to load: HTTP ${response.status}`);
  const buffer = await context.decodeAudioData(await response.arrayBuffer());
  if (buffer.numberOfChannels !== 1) throw new Error(`Recording ${url} must be mono, found ${buffer.numberOfChannels} channels`);
  return buffer;
}

export async function loadSoundRecordings(context: BaseAudioContext): Promise<SoundRecordings> {
  return { screams: await loadScreamRecordings(context) };
}

async function loadScreamRecordings(context: BaseAudioContext): Promise<ScreamRecording[]> {
  return Promise.all(SCREAM_RECORDING_URLS.map(async (url) => {
    const buffer = await decodeMono(context, url);
    const playableSeconds = buffer.duration / (1 + SOUND_TUNING.scream.pitchSpread);
    if (playableSeconds < longestScreamSeconds()) throw new Error(`Scream recording ${url} lasts ${buffer.duration.toFixed(2)} s, too short for the longest scream`);
    const samples = buffer.getChannelData(0);
    let energy = 0;
    for (const sample of samples) energy += sample * sample;
    return { buffer, normalization: 1 / Math.sqrt(energy / samples.length) };
  }));
}
