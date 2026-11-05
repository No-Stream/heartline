import type { TrainSound } from "../mapping";
import type { ScheduleClock } from "../engine";
import { noiseBuffers } from "./noise";

export function createTrainVoice(context: BaseAudioContext, destination: AudioNode) {
  const { white, pink } = noiseBuffers(context);
  const output = context.createGain();
  const pan = context.createStereoPanner();
  const distanceFilter = context.createBiquadFilter(); distanceFilter.type = "lowpass"; distanceFilter.frequency.value = 8000;
  output.connect(distanceFilter).connect(pan).connect(destination);
  const noise = (buffer: AudioBuffer): AudioBufferSourceNode => {
    const source = context.createBufferSource(); source.buffer = buffer; source.loop = true; source.start(); return source;
  };
  const oscillator = (type: OscillatorType, frequency: number): OscillatorNode => {
    const source = context.createOscillator(); source.type = type; source.frequency.value = frequency; source.start(); return source;
  };
  const filter = (type: BiquadFilterType, frequency: number, q = 0.7): BiquadFilterNode => {
    const node = context.createBiquadFilter(); node.type = type; node.frequency.value = frequency; node.Q.value = q; return node;
  };
  const gain = (value = 0): GainNode => { const node = context.createGain(); node.gain.value = value; return node; };
  const pinkSource = noise(pink);
  const band = filter("bandpass", 200); const low = filter("lowpass", 600); const roar = gain();
  pinkSource.connect(band).connect(low).connect(roar).connect(output);
  const rumbleFilter = filter("lowpass", 120, 4); const rumble = gain();
  pinkSource.connect(rumbleFilter).connect(rumble).connect(output);
  const tremolo = oscillator("triangle", 1); const tremoloDepth = gain(); tremolo.connect(tremoloDepth).connect(rumble.gain);
  const whine = oscillator("sawtooth", 180); const whineFilter = filter("highpass", 1500); const whineGain = gain();
  whine.connect(whineFilter).connect(whineGain).connect(output);
  const motorFilter = filter("lowpass", 300); const motor = gain();
  oscillator("triangle", 50).connect(motorFilter); oscillator("triangle", 100).connect(motorFilter); motorFilter.connect(motor).connect(output);
  const brakeFilter = filter("highpass", 2500); const brakeGain = gain(); noise(white).connect(brakeFilter).connect(brakeGain).connect(output);
  const squeal = oscillator("sine", 2200); const vibrato = oscillator("sine", 6); const vibratoDepth = gain(30); vibrato.connect(vibratoDepth).connect(squeal.frequency);
  const squealGain = gain(); squeal.connect(squealGain).connect(output);
  let oneShots: { envelope: GainNode; end: number }[] = [];
  const trackShot = (envelope: GainNode, end: number): void => { oneShots.push({ envelope, end }); };
  let wasMuted = false;
  function silenceShots(time: number): void {
    for (const shot of oneShots) { shot.envelope.gain.cancelScheduledValues(time); shot.envelope.gain.setTargetAtTime(0, time, 0.01); }
    oneShots = [];
  }
  let nextClack = 0; let clackIndex = 0; let brakeStart: number | null = null;
  const target = (parameter: AudioParam, value: number, time: number, smoothing = 0.05): void => { parameter.setTargetAtTime(value, time, smoothing); };
  function clack(time: number, level: number): void {
    const source = context.createBufferSource(); source.buffer = white;
    const clackFilter = filter("bandpass", 1800, 2); const envelope = gain();
    source.connect(clackFilter).connect(envelope).connect(output);
    envelope.gain.setValueAtTime(0.001, time); envelope.gain.linearRampToValueAtTime(0.25 * level + 0.001, time + 0.001); envelope.gain.exponentialRampToValueAtTime(0.001, time + 0.026);
    trackShot(envelope, time + 0.03);
    source.start(time, (clackIndex++ * 0.137) % (white.duration - 0.03), 0.03);
  }
  function launch(time: number, level: number): void {
    const source = noise(white); const sweep = filter("bandpass", 300, 1); const envelope = gain();
    source.connect(sweep).connect(envelope).connect(output); sweep.frequency.setValueAtTime(300, time); sweep.frequency.exponentialRampToValueAtTime(3000, time + 1.2);
    envelope.gain.setValueAtTime(0, time); envelope.gain.linearRampToValueAtTime(0.35 * level, time + 0.08); envelope.gain.setValueAtTime(0.35 * level, time + 0.48); envelope.gain.linearRampToValueAtTime(0, time + 1.08); source.stop(time + 1.2); trackShot(envelope, time + 1.2);
    const tone = context.createOscillator(); tone.frequency.setValueAtTime(80, time); tone.frequency.exponentialRampToValueAtTime(400, time + 1.2); const toneGain = gain(); tone.connect(toneGain).connect(output);
    toneGain.gain.setValueAtTime(0, time); toneGain.gain.linearRampToValueAtTime(0.15 * level, time + 0.08); toneGain.gain.linearRampToValueAtTime(0, time + 1.2); tone.start(time); tone.stop(time + 1.2); trackShot(toneGain, time + 1.2);
  }
  function arrival(time: number, level: number): void {
    const source = noise(pink); const air = filter("lowpass", 1500); const envelope = gain(); source.connect(air).connect(envelope).connect(output);
    envelope.gain.setValueAtTime(Math.max(0.001, 0.2 * level), time); envelope.gain.exponentialRampToValueAtTime(0.001, time + 0.4); source.stop(time + 0.4); trackShot(envelope, time + 0.4);
  }
  function departure(time: number, level: number): void {
    for (const [index, frequency] of [880, 1175].entries()) {
      const start = time + index * 0.12; const tone = context.createOscillator(); tone.frequency.value = frequency; const envelope = gain(); tone.connect(envelope).connect(output);
      envelope.gain.setValueAtTime(0, start); envelope.gain.linearRampToValueAtTime(0.12 * level, start + 0.01); envelope.gain.setValueAtTime(0.12 * level, start + 0.12); envelope.gain.linearRampToValueAtTime(0, start + 0.27); tone.start(start); tone.stop(start + 0.27); trackShot(envelope, start + 0.27);
    }
  }
  return {
    apply(train: TrainSound, clock: ScheduleClock, eventsMuted: boolean): void {
      const { now, until } = clock; oneShots = oneShots.filter((shot) => shot.end > now);
      if (eventsMuted && !wasMuted) silenceShots(now);
      wasMuted = eventsMuted; const level = train.audibility;
      const roarLevel = train.speed < 0.5 ? 0 : 0.5 * Math.min(1, (train.speed / 30) ** 1.5);
      target(pan.pan, train.pan, now); target(distanceFilter.frequency, 1200 + 6800 / (1 + ((train.distance ?? 0) / 440) ** 2), now); target(band.frequency, 200 + 40 * train.speed, now); target(low.frequency, 600 + 150 * train.speed, now);
      target(roar.gain, level * roarLevel, now); target(whine.frequency, 180 + 25 * train.speed, now);
      target(whineGain.gain, train.material === "wood" ? 0 : 0.02 * level * roarLevel, now);
      const woodLevel = train.material === "steel" ? 0 : train.material === "wood" ? 1 : 0.5;
      target(rumble.gain, 0.6 * level * roarLevel * woodLevel, now); target(tremolo.frequency, Math.max(0.01, train.speed / 1.2), now);
      target(tremoloDepth.gain, 0.35 * 0.6 * level * roarLevel * woodLevel, now);
      const lifting = train.onLift && train.speed > 0.1 && !eventsMuted;
      target(motor.gain, lifting ? 0.05 * level : 0, now, 0.3);
      if (lifting && train.clackRate > 0) {
        nextClack = Math.max(nextClack, now);
        while (nextClack < until) { clack(nextClack, level); nextClack += 1 / train.clackRate; }
      } else nextClack = now;
      if (!eventsMuted && train.launch) launch(now, level);
      if (!eventsMuted && train.arrive) arrival(now, level);
      if (!eventsMuted && train.depart) departure(now, level);
      if (!eventsMuted && train.brake > 0) brakeStart ??= now; else brakeStart = null;
      const braking = brakeStart !== null && now - brakeStart < 1.5 ? train.brake * level : 0;
      target(brakeGain.gain, 0.3 * braking, now, braking ? 0.05 : 0.3); target(squealGain.gain, 0.03 * braking, now, braking ? 0.05 : 0.3);
    },
    release(clock: ScheduleClock): void {
      for (const parameter of [roar.gain, rumble.gain, tremoloDepth.gain, whineGain.gain, motor.gain, brakeGain.gain, squealGain.gain]) target(parameter, 0, clock.now);
      silenceShots(clock.now); nextClack = clock.now; brakeStart = null; wasMuted = false;
    },
  };
}
