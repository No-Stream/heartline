import "./toolbar.css";
import type { InputAction } from "./input/actions";

export interface SoundPreference {
  volume: number;
  muted: boolean;
}

const storageKey = "theme-park-sim.sound";
const defaultPreference: SoundPreference = { volume: 0.7, muted: false };

export function parseStoredSound(text: string): SoundPreference | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    if (error instanceof SyntaxError) return undefined;
    throw error;
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return undefined;
  const preference = parsed as Record<string, unknown>;
  if (typeof preference.volume !== "number" || !Number.isFinite(preference.volume) || preference.volume < 0 || preference.volume > 1) return undefined;
  if (typeof preference.muted !== "boolean") return undefined;
  return { volume: preference.volume, muted: preference.muted };
}

function storedSound(): SoundPreference | undefined {
  try {
    const stored = localStorage.getItem(storageKey);
    return stored === null ? undefined : parseStoredSound(stored);
  } catch (error) {
    if (error instanceof DOMException) return undefined;
    throw error;
  }
}

function storeSound(preference: SoundPreference): void {
  try {
    localStorage.setItem(storageKey, JSON.stringify(preference));
  } catch (error) {
    if (!(error instanceof DOMException)) throw error;
  }
}

/** A compact game-bar control for the master sound level and mute preference. */
export function createSoundSetting(
  container: HTMLElement,
  dispatch: (action: InputAction) => void,
): SoundPreference & { setMuted(muted: boolean): void; setVolume(volume: number): void; setLocked(locked: boolean): void } {
  const preference = storedSound() ?? { ...defaultPreference };
  container.innerHTML = `
    <div class="sound-setting">
      <button class="theme-toggle sound-setting__mute" type="button" aria-label="Sound" aria-pressed="true" data-sound="on">
        <svg class="toolbar__icon" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
          <path class="toolbar__icon-line" d="M3 8v4h3l4 3V5L6 8H3z" />
          <path class="sound-setting__waves" d="M12 7a4 4 0 0 1 0 6M14 5a7 7 0 0 1 0 10" />
          <path class="sound-setting__muted-mark" d="m12 8 5 5m0-5-5 5" />
        </svg>
      </button>
      <input class="sound-setting__volume" type="range" min="0" max="100" step="1" aria-label="Volume">
    </div>`;
  const button = container.querySelector<HTMLButtonElement>("[data-sound]");
  const slider = container.querySelector<HTMLInputElement>("[aria-label='Volume']");
  if (!button || !slider) throw new Error("Sound controls are missing");

  let locked = true;
  const show = (): void => {
    const soundOn = !preference.muted;
    button.dataset.sound = soundOn ? "on" : "off";
    button.setAttribute("aria-pressed", String(soundOn));
    button.title = locked ? "Sound starts after your first click." : soundOn ? "Sound on. Press M to mute." : "Sound muted. Press M to turn sound on.";
    slider.value = String(Math.round(preference.volume * 100));
  };
  show();

  button.addEventListener("click", () => dispatch({ type: "sound-mute-toggle" }));
  slider.addEventListener("input", () => dispatch({ type: "sound-volume", volume: Number(slider.value) / 100 }));

  return {
    get volume() { return preference.volume; },
    get muted() { return preference.muted; },
    setLocked(value) { locked = value; show(); },
    setMuted(muted) {
      preference.muted = muted;
      storeSound(preference);
      show();
    },
    setVolume(volume) {
      if (!Number.isFinite(volume) || volume < 0 || volume > 1) throw new RangeError("Sound volume must be between 0 and 1");
      preference.volume = volume;
      storeSound(preference);
      show();
    },
  };
}
