import type { SoundFrame } from "../mapping";
import type { ScheduleClock } from "../engine";
import { noiseBuffers } from "./noise";

export function createAmbienceVoice(context: BaseAudioContext, destination: AudioNode) {
  const wind = context.createBufferSource(); wind.buffer = noiseBuffers(context).pink; wind.loop = true;
  const windFilter = context.createBiquadFilter(); windFilter.type = "lowpass"; windFilter.frequency.value = 400;
  const windModulation = context.createGain(); windModulation.gain.value = 1;
  const windLevel = context.createGain(); windLevel.gain.value = 0;
  wind.connect(windFilter).connect(windModulation).connect(windLevel).connect(destination);
  const windLfo = context.createOscillator(); windLfo.frequency.value = 0.1;
  const windDepth = context.createGain(); windDepth.gain.value = 0.4; windLfo.connect(windDepth).connect(windModulation.gain);
  wind.start(); windLfo.start();

  const dayLevel = context.createGain(); dayLevel.gain.value = 0; dayLevel.connect(destination);
  const cricket = context.createOscillator(); cricket.type = "square"; cricket.frequency.value = 4200;
  const cricketFilter = context.createBiquadFilter(); cricketFilter.type = "bandpass"; cricketFilter.frequency.value = 4200; cricketFilter.Q.value = 8;
  const chirring = context.createGain(); chirring.gain.value = 0.5;
  const interval = context.createGain(); interval.gain.value = 0.5;
  const nightLevel = context.createGain(); nightLevel.gain.value = 0;
  cricket.connect(cricketFilter).connect(chirring).connect(interval).connect(nightLevel).connect(destination);
  const chirringLfo = context.createOscillator(); chirringLfo.type = "square"; chirringLfo.frequency.value = 25;
  const chirringDepth = context.createGain(); chirringDepth.gain.value = 0.5; chirringLfo.connect(chirringDepth).connect(chirring.gain);
  const intervalLfo = context.createOscillator(); intervalLfo.frequency.value = 1 / 2.3;
  const real = new Float32Array(33); const imaginary = new Float32Array(33); const duty = 1.5 / 2.3;
  for (let harmonic = 1; harmonic < real.length; harmonic++) {
    real[harmonic] = Math.sin(2 * Math.PI * harmonic * duty) / (Math.PI * harmonic);
    imaginary[harmonic] = (1 - Math.cos(2 * Math.PI * harmonic * duty)) / (Math.PI * harmonic);
  }
  intervalLfo.setPeriodicWave(context.createPeriodicWave(real, imaginary, { disableNormalization: true }));
  interval.gain.value = duty; intervalLfo.connect(interval.gain);
  cricket.start(); chirringLfo.start(); intervalLfo.start();

  let seed = 1; let nextBird = 0;
  const random = (): number => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  function bird(time: number): void {
    const pan = context.createStereoPanner(); pan.pan.value = random() * 1.6 - 0.8; pan.connect(dayLevel);
    const count = 2 + Math.floor(random() * 3);
    for (let index = 0; index < count; index++) {
      const start = time + index * 0.12;
      const oscillator = context.createOscillator(); oscillator.frequency.setValueAtTime(2500, start); oscillator.frequency.linearRampToValueAtTime(3500, start + 0.08);
      const envelope = context.createGain(); envelope.gain.setValueAtTime(0, start); envelope.gain.linearRampToValueAtTime(0.06, start + 0.005); envelope.gain.setValueAtTime(0.06, start + 0.02); envelope.gain.linearRampToValueAtTime(0, start + 0.08);
      oscillator.connect(envelope).connect(pan); oscillator.start(start); oscillator.stop(start + 0.08);
    }
  }
  return {
    apply(ambience: SoundFrame["ambience"], clock: ScheduleClock): void {
      windLevel.gain.setTargetAtTime(0.08 * ambience.distance, clock.now, 0.05);
      dayLevel.gain.setTargetAtTime(ambience.day, clock.now, 0.05);
      nightLevel.gain.setTargetAtTime(0.04 * ambience.night, clock.now, 0.05);
      if (ambience.day > 0) {
        nextBird = Math.max(clock.now, nextBird);
        while (nextBird < clock.until) { bird(nextBird); nextBird += 1 + 3 * random(); }
      } else nextBird = clock.now;
    },
  };
}
