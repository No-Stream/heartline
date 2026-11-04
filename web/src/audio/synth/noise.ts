const buffers = new WeakMap<BaseAudioContext, { white: AudioBuffer; pink: AudioBuffer }>();

export function noiseSamples(sampleRate: number): { white: Float32Array; pink: Float32Array } {
  const white = new Float32Array(sampleRate * 2);
  const pink = new Float32Array(white.length);
  let seed = 0x12345678;
  let first = 0; let second = 0; let third = 0;
  for (let index = 0; index < white.length; index++) {
    seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5;
    const sample = (seed >>> 0) / 0x80000000 - 1;
    white[index] = sample;
    first = 0.99765 * first + sample * 0.0990460;
    second = 0.96300 * second + sample * 0.2965164;
    third = 0.57000 * third + sample * 1.0526913;
    pink[index] = (first + second + third + sample * 0.1848) * 0.1;
  }
  return { white, pink };
}

export function noiseBuffers(context: BaseAudioContext): { white: AudioBuffer; pink: AudioBuffer } {
  const existing = buffers.get(context);
  if (existing) return existing;
  const samples = noiseSamples(context.sampleRate);
  const white = context.createBuffer(1, samples.white.length, context.sampleRate);
  const pink = context.createBuffer(1, samples.pink.length, context.sampleRate);
  white.getChannelData(0).set(samples.white);
  pink.getChannelData(0).set(samples.pink);
  const result = { white, pink };
  buffers.set(context, result);
  return result;
}
