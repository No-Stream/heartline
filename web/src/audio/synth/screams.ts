import type { ScreamEvent } from "../mapping";
import { noiseBuffers } from "./noise";

export interface ScreamHandle { start: number; end: number; stop(time: number): void }

export function playScream(context: BaseAudioContext, destination: AudioNode, event: ScreamEvent, time: number, variation: () => number): ScreamHandle {
  const output = context.createGain();
  const pan = context.createStereoPanner();
  pan.pan.value = event.pan;
  output.connect(pan).connect(destination);
  const count = 3 + Math.round(3 * event.intensity);
  const hold = 0.6 + 0.6 * event.intensity;
  const voiceLevel = 0.18 * event.audibility / count;
  let end = time;
  const pink = noiseBuffers(context).pink;
  for (let index = 0; index < count; index++) {
    const start = time + variation() * 0.08;
    const release = start + 0.06 + hold;
    const voiceEnd = release + 0.3;
    end = Math.max(end, voiceEnd);
    const voice = context.createOscillator();
    voice.type = "sawtooth";
    voice.detune.value = (variation() * 2 - 1) * 40;
    voice.frequency.setValueAtTime(500, start);
    voice.frequency.linearRampToValueAtTime(650, start + 0.3);
    const vibrato = context.createOscillator();
    vibrato.frequency.value = 5.5;
    const vibratoDepth = context.createGain();
    vibratoDepth.gain.value = 20;
    vibrato.connect(vibratoDepth).connect(voice.frequency);
    const envelope = context.createGain();
    envelope.gain.setValueAtTime(0, start);
    envelope.gain.linearRampToValueAtTime(voiceLevel, start + 0.06);
    envelope.gain.setValueAtTime(voiceLevel, release);
    envelope.gain.linearRampToValueAtTime(0, voiceEnd);
    envelope.connect(output);
    for (const frequency of [700, 1200]) {
      const formant = context.createBiquadFilter();
      formant.type = "bandpass"; formant.frequency.value = frequency; formant.Q.value = 5;
      voice.connect(formant).connect(envelope);
    }
    const breath = context.createBufferSource();
    breath.buffer = pink; breath.loop = true;
    const breathFilter = context.createBiquadFilter();
    breathFilter.type = "bandpass"; breathFilter.frequency.value = 2000;
    const breathGain = context.createGain(); breathGain.gain.value = 0.1;
    breath.connect(breathFilter).connect(breathGain).connect(envelope);
    voice.start(start); voice.stop(voiceEnd);
    vibrato.start(start); vibrato.stop(voiceEnd);
    breath.start(start); breath.stop(voiceEnd);
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
