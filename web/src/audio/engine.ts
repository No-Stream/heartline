import { allocateTrainVoices, MAX_SCREAM_GROUPS, MAX_TRAIN_VOICES, MAX_UI_VOICES } from "./voices";
import type { SoundFrame, UiSound } from "./mapping";
import { createTrainVoice } from "./synth/train";
import { playUiSound, type UiSoundHandle } from "./synth/ui";

export const AUDIO_LOOKAHEAD = 0.15;
export interface ScheduleClock { now: number; until: number }
export function scheduleClock(now: number): ScheduleClock { return { now, until: now + AUDIO_LOOKAHEAD }; }
export function schedulePeriodic(nextTime: number, rate: number, clock: ScheduleClock): { events: number[]; nextTime: number } {
  if (!Number.isFinite(rate) || rate <= 0) throw new RangeError("Event rate must be positive and finite");
  const events: number[] = [];
  nextTime = Math.max(nextTime, clock.now);
  while (nextTime < clock.until) { events.push(nextTime); nextTime += 1 / rate; }
  return { events, nextTime };
}

export interface OfflineScript { duration: number; frames: { time: number; frame: SoundFrame }[]; volume?: number; muted?: boolean; includeSamples?: boolean }
/** `peakTime` is when the loudest sample plays, in seconds; `above1000` overlaps the other bands and uses the same one-pole split. */
export interface ChannelMetrics { peak: number; peakTime: number; rms: number; bands: { below200: number; from200To2000: number; above2000: number; above1000: number }; samples?: number[] }
export interface OfflineResult { channels: ChannelMetrics[] }
export interface ActiveVoices { trains: number; screams: number; ui: number; ambience: "day" | "night" | "silent" }

export function createAudioEngine(context: BaseAudioContext, volume = 0.7, muted = false) {
  const master = context.createGain();
  const compressor = context.createDynamicsCompressor();
  compressor.threshold.value = -12;
  compressor.ratio.value = 3;
  master.gain.value = muted ? 0 : volume * volume;
  master.connect(compressor).connect(context.destination);
  const trainPool = Array.from({ length: MAX_TRAIN_VOICES }, () => context.createGain());
  const trainVoices = trainPool.map((voice) => createTrainVoice(context, voice));
  const screamPool = Array.from({ length: MAX_SCREAM_GROUPS }, () => context.createGain());
  const uiPool = Array.from({ length: MAX_UI_VOICES }, () => context.createGain());
  const crowd = context.createGain();
  const ambience = context.createGain();
  for (const voice of [...trainPool, ...screamPool, ...uiPool, crowd, ambience]) voice.connect(master);
  let allocation: (number | null)[] = [];
  const uiSounds: (UiSoundHandle | null)[] = uiPool.map(() => null);
  let nextUiSlot = 0;
  let variationSeed = 1;
  const variation = (): number => { variationSeed = (variationSeed * 16807) % 2147483647; return (variationSeed - 1) / 2147483646; };
  const emptyFamily = { apply(_parameters: unknown, _clock: ScheduleClock): void {}, trigger(_event: unknown, _clock: ScheduleClock): void {}, release(_clock: ScheduleClock): void {} };
  return {
    setVolume(nextVolume: number, nextMuted: boolean, atTime = context.currentTime): void {
      master.gain.setTargetAtTime(nextMuted ? 0 : nextVolume * nextVolume, atTime, 0.05);
    },
    fadeOut(atTime = context.currentTime): void {
      master.gain.cancelScheduledValues(atTime);
      master.gain.setValueAtTime(master.gain.value, atTime);
      master.gain.linearRampToValueAtTime(0, atTime + 0.05);
    },
    apply(frame: SoundFrame, atTime = context.currentTime): void {
      const clock = scheduleClock(atTime);
      const previous = allocation;
      allocation = allocateTrainVoices(allocation, frame.trains);
      for (let index = 0; index < trainPool.length; index++) {
        if (previous[index] !== undefined && previous[index] !== null && previous[index] !== allocation[index]) trainVoices[index]!.release(clock);
        const train = frame.trains.find((candidate) => candidate.train === allocation[index]);
        if (train) trainVoices[index]!.apply(train, clock, frame.eventsMuted);
        trainPool[index]!.gain.setTargetAtTime(allocation[index] === null ? 0 : 1, atTime, 0.05);
      }
      emptyFamily.apply(frame.crowd, clock);
      emptyFamily.apply(frame.ambience, clock);
      if (!frame.eventsMuted) for (const event of frame.screams) emptyFamily.trigger(event, clock);
      if (frame.paused) {
        for (let index = 0; index < uiSounds.length; index++) {
          uiSounds[index]?.stop(clock.now);
          uiSounds[index] = null;
        }
      }
      // A fifth overlapping interface sound fades out the oldest, so a burst of events cannot stack up.
      for (const event of frame.ui) {
        uiSounds[nextUiSlot]?.stop(clock.now);
        uiSounds[nextUiSlot] = playUiSound(context, uiPool[nextUiSlot]!, event, clock.now, variation);
        nextUiSlot = (nextUiSlot + 1) % uiPool.length;
      }
    },
    activeVoices(): ActiveVoices {
      const ui = uiSounds.filter((sound) => sound !== null && sound.end > context.currentTime).length;
      return { trains: allocation.filter((train) => train !== null).length, screams: 0, ui, ambience: "silent" };
    },
  };
}

export function createLiveAudio(options: { testMode: boolean; onLockChange(locked: boolean): void }) {
  let context: AudioContext | undefined;
  let engine: ReturnType<typeof createAudioEngine> | undefined;
  let volume = 0.7;
  let muted = false;
  let lastUiEvent: UiSound | null = null;
  let hiddenTimer: ReturnType<typeof setTimeout> | undefined;
  let transition = Promise.resolve();
  function syncState(): Promise<void> {
    transition = transition.then(async () => {
      if (!context || !engine) return;
      if (muted || document.hidden) {
        engine.setVolume(volume, true);
        if (context.state === "running") await context.suspend();
      } else {
        await context.resume();
        engine.setVolume(volume, false);
      }
    });
    return transition;
  }
  function removeUnlockListeners(): void {
    document.removeEventListener("pointerdown", gesture, true);
    document.removeEventListener("keydown", gesture, true);
  }
  function unlock(): Promise<void> {
    removeUnlockListeners();
    if (!context) {
      context = new AudioContext();
      engine = createAudioEngine(context, volume, muted);
      options.onLockChange(false);
      if (!muted && !document.hidden) transition = context.resume();
    }
    return syncState();
  }
  function gesture(): void { void unlock(); }
  if (!options.testMode) {
    document.addEventListener("pointerdown", gesture, true);
    document.addEventListener("keydown", gesture, true);
  }
  options.onLockChange(true);
  document.addEventListener("visibilitychange", () => {
    if (!context || !engine) return;
    if (hiddenTimer !== undefined) clearTimeout(hiddenTimer);
    if (document.hidden) {
      engine.fadeOut();
      hiddenTimer = setTimeout(() => { hiddenTimer = undefined; void syncState(); }, 50);
    } else void syncState();
  });
  return {
    state(): "locked" | AudioContextState { return context?.state ?? "locked"; },
    unlock,
    setVolume(nextVolume: number, nextMuted: boolean): void {
      if (!Number.isFinite(nextVolume) || nextVolume < 0 || nextVolume > 1) throw new RangeError("Sound volume must be between 0 and 1");
      volume = nextVolume; muted = nextMuted;
      if (context) void syncState();
    },
    apply(frame: SoundFrame): void {
      lastUiEvent = frame.ui.at(-1)?.sound ?? lastUiEvent;
      engine?.apply(frame);
    },
    /** The most recent interface sound requested, recorded even while the context is locked, so tests can check triggers silently. */
    lastUiEvent(): UiSound | null { return lastUiEvent; },
    activeVoices(): ActiveVoices { return engine?.activeVoices() ?? { trains: 0, screams: 0, ui: 0, ambience: "silent" }; },
  };
}

export async function renderOffline(script: OfflineScript): Promise<OfflineResult> {
  if (!Number.isFinite(script.duration) || script.duration <= 0 || script.duration > 4) throw new RangeError("Offline audio duration must be between 0 and 4 seconds");
  const sampleRate = 48000;
  const context = new OfflineAudioContext(2, Math.ceil(script.duration * sampleRate), sampleRate);
  const engine = createAudioEngine(context, script.volume ?? 0.7, script.muted ?? false);
  for (const entry of script.frames) {
    if (!Number.isFinite(entry.time) || entry.time < 0 || entry.time > script.duration) throw new RangeError("Invalid offline frame time");
    engine.apply(entry.frame, entry.time);
  }
  const rendered = await context.startRendering();
  const channels: ChannelMetrics[] = [];
  for (let channel = 0; channel < rendered.numberOfChannels; channel++) {
    const samples = rendered.getChannelData(channel);
    let peak = 0; let peakIndex = 0; let energy = 0; let low = 0; let midLow = 0; let kiloLow = 0;
    let lowEnergy = 0; let midEnergy = 0; let highEnergy = 0; let aboveKiloEnergy = 0;
    const lowAlpha = 1 - Math.exp(-2 * Math.PI * 200 / sampleRate);
    const midAlpha = 1 - Math.exp(-2 * Math.PI * 2000 / sampleRate);
    const kiloAlpha = 1 - Math.exp(-2 * Math.PI * 1000 / sampleRate);
    for (let index = 0; index < samples.length; index++) {
      const sample = samples[index]!;
      if (!Number.isFinite(sample)) throw new Error("Offline render contains a non-finite sample");
      if (Math.abs(sample) > peak) { peak = Math.abs(sample); peakIndex = index; }
      energy += sample * sample;
      low += lowAlpha * (sample - low); midLow += midAlpha * (sample - midLow); kiloLow += kiloAlpha * (sample - kiloLow);
      lowEnergy += low * low; midEnergy += (midLow - low) ** 2; highEnergy += (sample - midLow) ** 2; aboveKiloEnergy += (sample - kiloLow) ** 2;
    }
    const bandRms = (bandEnergy: number): number => Math.sqrt(bandEnergy / samples.length);
    channels.push({ ...(script.includeSamples ? { samples: Array.from(samples) } : {}), peak, peakTime: peakIndex / sampleRate, rms: bandRms(energy), bands: { below200: bandRms(lowEnergy), from200To2000: bandRms(midEnergy), above2000: bandRms(highEnergy), above1000: bandRms(aboveKiloEnergy) } });
  }
  return { channels };
}
