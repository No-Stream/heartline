import { SOUND_TUNING, type ScreamEvent } from "../mapping";
import type { ScreamRecording } from "../recordings";

export interface ScreamHandle { start: number; end: number; stop(time: number): void }

export function playScream(context: BaseAudioContext, destination: AudioNode, event: ScreamEvent, time: number, variation: () => number, recordings: readonly ScreamRecording[]): ScreamHandle {
  const tuning = SOUND_TUNING.scream;
  const output = context.createGain();
  output.connect(destination);
  const hold = tuning.hold[0] + tuning.hold[1] * event.intensity;
  const level = tuning.level * event.audibility * (0.6 + 0.4 * event.intensity);
  const firstRecording = Math.floor(variation() * recordings.length);
  const layers = [{ recording: firstRecording, start: time, level, pan: event.pan }];
  if (event.intensity >= tuning.layerIntensity) {
    layers.push({
      recording: (firstRecording + 1 + Math.floor(variation() * (recordings.length - 1))) % recordings.length,
      start: time + tuning.layerDelay[0] + (tuning.layerDelay[1] - tuning.layerDelay[0]) * variation(),
      level: level * tuning.layerLevel,
      pan: Math.max(-1, Math.min(1, event.pan + (variation() * 2 - 1) * tuning.layerPanSpread)),
    });
  }
  let end = time;
  for (const layer of layers) {
    const recording = recordings[layer.recording]!;
    const source = context.createBufferSource();
    source.buffer = recording.buffer;
    source.playbackRate.value = 1 + (variation() * 2 - 1) * tuning.pitchSpread;
    const envelope = context.createGain();
    const peak = layer.level * recording.normalization;
    const releaseStart = layer.start + tuning.attack + hold;
    const layerEnd = releaseStart + tuning.release;
    envelope.gain.setValueAtTime(0, layer.start);
    envelope.gain.linearRampToValueAtTime(peak, layer.start + tuning.attack);
    envelope.gain.setValueAtTime(peak, releaseStart);
    envelope.gain.linearRampToValueAtTime(0, layerEnd);
    const pan = context.createStereoPanner();
    pan.pan.value = layer.pan;
    source.connect(envelope).connect(pan).connect(output);
    source.start(layer.start);
    source.stop(layerEnd);
    end = Math.max(end, layerEnd);
  }
  return {
    start: time, end,
    stop(stopTime: number): void {
      output.gain.cancelScheduledValues(stopTime);
      output.gain.setValueAtTime(1, stopTime);
      output.gain.setTargetAtTime(0, stopTime, 0.01);
    },
  };
}
