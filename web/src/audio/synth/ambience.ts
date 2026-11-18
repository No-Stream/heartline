import { SOUND_TUNING, type SoundFrame } from "../mapping";
import type { ScheduleClock } from "../engine";
import { noiseBuffers } from "./noise";
import { birdGap, birdPhrase, createBirdSingers, playBirdPhrase } from "./birds";

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

  let seed = 1;
  const random = (): number => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
  const singers = createBirdSingers(random);
  const nextSong = singers.map(() => 0);
  let birdsSinging = false;
  return {
    apply(ambience: SoundFrame["ambience"], clock: ScheduleClock): void {
      windLevel.gain.setTargetAtTime(0.08 * ambience.distance, clock.now, 0.05);
      dayLevel.gain.setTargetAtTime(ambience.day, clock.now, 0.05);
      nightLevel.gain.setTargetAtTime(0.04 * ambience.night, clock.now, 0.05);
      if (ambience.day > 0) {
        if (!birdsSinging) for (let index = 0; index < singers.length; index++) nextSong[index] = clock.now + random() * SOUND_TUNING.birds.entrySpreadSeconds;
        birdsSinging = true;
        singers.forEach((singer, index) => {
          while (nextSong[index]! < clock.until) nextSong[index] = playBirdPhrase(context, dayLevel, singer, birdPhrase(singer, random), Math.max(clock.now, nextSong[index]!)) + birdGap(random);
        });
      } else birdsSinging = false;
    },
  };
}
