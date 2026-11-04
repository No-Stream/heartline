import { expect, it } from "vitest";
import { noiseSamples } from "./noise";

it("generates deterministic, approximately zero mean noise with more low frequency pink energy", () => {
  const { white, pink } = noiseSamples(48000);
  expect(noiseSamples(48000).white).toEqual(white);
  for (const samples of [white, pink]) expect(Math.abs(samples.reduce((sum, value) => sum + value, 0) / samples.length)).toBeLessThan(0.03);
  function lowFrequencyEnergy(samples: Float32Array): number {
    let energy = 0;
    for (let frequency = 25; frequency <= 200; frequency += 25) {
      let real = 0; let imaginary = 0;
      for (let index = 0; index < samples.length; index++) {
        const phase = 2 * Math.PI * frequency * index / 48000;
        real += samples[index]! * Math.cos(phase);
        imaginary += samples[index]! * Math.sin(phase);
      }
      energy += real * real + imaginary * imaginary;
    }
    return energy;
  }
  expect(lowFrequencyEnergy(pink)).toBeGreaterThan(lowFrequencyEnergy(white));
});
