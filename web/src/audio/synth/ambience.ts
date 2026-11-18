import type { SoundFrame } from "../mapping";
import type { ScheduleClock } from "../engine";
import { noiseBuffers } from "./noise";

/** Wind only, day and night: the owner removed the birds and crickets as too shrill on 2026-11-18. */
export function createAmbienceVoice(context: BaseAudioContext, destination: AudioNode) {
  const wind = context.createBufferSource(); wind.buffer = noiseBuffers(context).pink; wind.loop = true;
  const windFilter = context.createBiquadFilter(); windFilter.type = "lowpass"; windFilter.frequency.value = 400;
  const windModulation = context.createGain(); windModulation.gain.value = 1;
  const windLevel = context.createGain(); windLevel.gain.value = 0;
  wind.connect(windFilter).connect(windModulation).connect(windLevel).connect(destination);
  const windLfo = context.createOscillator(); windLfo.frequency.value = 0.1;
  const windDepth = context.createGain(); windDepth.gain.value = 0.4; windLfo.connect(windDepth).connect(windModulation.gain);
  wind.start(); windLfo.start();
  return {
    apply(ambience: SoundFrame["ambience"], clock: ScheduleClock): void {
      windLevel.gain.setTargetAtTime(0.08 * ambience.distance, clock.now, 0.05);
    },
  };
}
