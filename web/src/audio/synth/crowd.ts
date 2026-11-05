import type { SoundFrame } from "../mapping";
import type { ScheduleClock } from "../engine";
import { noiseBuffers } from "./noise";

export function createCrowdVoice(context: BaseAudioContext, destination: AudioNode) {
  const output = context.createGain(); output.gain.value = 0;
  const pan = context.createStereoPanner(); output.connect(pan).connect(destination);
  const firstFormant = context.createBiquadFilter(); firstFormant.type = "bandpass"; firstFormant.frequency.value = 300; firstFormant.Q.value = 0.8;
  const secondFormant = context.createBiquadFilter(); secondFormant.type = "bandpass"; secondFormant.frequency.value = 1200; secondFormant.Q.value = 0.8;
  firstFormant.connect(secondFormant).connect(output);
  const pink = context.createBufferSource(); pink.buffer = noiseBuffers(context).pink; pink.loop = true; pink.connect(firstFormant); pink.start();
  const formantLfo = context.createOscillator(); formantLfo.frequency.value = 0.2;
  for (const [parameter, depth] of [[firstFormant.frequency, 45], [secondFormant.frequency, 180]] as const) {
    const modulation = context.createGain(); modulation.gain.value = depth; formantLfo.connect(modulation).connect(parameter);
  }
  formantLfo.start();
  for (const [index, frequency] of [160, 195, 230, 260].entries()) {
    const oscillator = context.createOscillator(); oscillator.type = "triangle"; oscillator.frequency.value = frequency;
    const level = context.createGain(); level.gain.value = 0.25 / 4; oscillator.connect(level).connect(firstFormant);
    const lfo = context.createOscillator(); lfo.frequency.value = 0.1 + index * 0.06;
    const depth = context.createGain(); depth.gain.value = 30; lfo.connect(depth).connect(oscillator.frequency);
    oscillator.start(); lfo.start();
  }
  return {
    apply(crowd: SoundFrame["crowd"], clock: ScheduleClock): void {
      output.gain.setTargetAtTime(crowd.level, clock.now, 0.05);
      pan.pan.setTargetAtTime(crowd.pan, clock.now, 0.05);
    },
  };
}
