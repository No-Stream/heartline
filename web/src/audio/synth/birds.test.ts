import { describe, expect, it } from "vitest";
import { birdGap, birdPhrase, createBirdSingers, type BirdNote } from "./birds";
import { SOUND_TUNING } from "../mapping";

function seededRandom(seed: number): () => number {
  return () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
}
const meanFrequency = (note: BirdNote): number => (note.fromFrequency + note.toFrequency) / 2;
const spread = (values: number[]): number => {
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
  return Math.sqrt(values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length) / mean;
};

describe("bird song", () => {
  const random = seededRandom(7);
  const singers = createBirdSingers(random);
  const phrases = singers.map((singer) => Array.from({ length: 30 }, () => birdPhrase(singer, random)));

  it("uses several differently placed singers with different song styles", () => {
    expect(singers).toHaveLength(SOUND_TUNING.birds.singers);
    expect(new Set(singers.map((singer) => singer.style)).size).toBeGreaterThanOrEqual(2);
    expect(new Set(singers.map((singer) => singer.pan.toFixed(2))).size).toBe(singers.length);
  });

  it("varies pitch within and across phrases", () => {
    const allFrequencies = phrases.flat(2).map(meanFrequency);
    expect(Math.min(...allFrequencies)).toBeGreaterThan(1500);
    expect(Math.max(...allFrequencies)).toBeLessThan(8000);
    expect(Math.max(...allFrequencies) / Math.min(...allFrequencies)).toBeGreaterThan(2);
    for (const singerPhrases of phrases) {
      const firstNotePitches = singerPhrases.map((phrase) => meanFrequency(phrase[0]!));
      expect(spread(firstNotePitches)).toBeGreaterThan(0.02);
      const multiNote = singerPhrases.filter((phrase) => phrase.length > 1);
      const pitchChanging = multiNote.filter((phrase) => Math.max(...phrase.map(meanFrequency)) / Math.min(...phrase.map(meanFrequency)) > 1.08);
      expect(pitchChanging.length).toBeGreaterThan(multiNote.length / 2);
    }
    expect(phrases.flat(2).some((note) => Math.abs(note.toFrequency / note.fromFrequency - 1) > 0.1)).toBe(true);
  });

  it("varies rhythm, note count and phrase length, and never repeats a phrase exactly", () => {
    const intervals = phrases.flat().flatMap((phrase) => phrase.slice(1).map((note, index) => note.start - phrase[index]!.start));
    expect(spread(intervals)).toBeGreaterThan(0.25);
    for (const singerPhrases of phrases) {
      expect(new Set(singerPhrases.map((phrase) => phrase.length)).size).toBeGreaterThan(1);
      for (let index = 1; index < singerPhrases.length; index++) expect(singerPhrases[index]).not.toEqual(singerPhrases[index - 1]);
    }
    const gaps = Array.from({ length: 200 }, () => birdGap(random));
    expect(Math.min(...gaps)).toBeGreaterThanOrEqual(SOUND_TUNING.birds.gapSeconds[0]);
    expect(Math.max(...gaps)).toBeLessThanOrEqual(SOUND_TUNING.birds.gapSeconds[1]);
    expect(spread(gaps)).toBeGreaterThan(0.2);
  });

  it("keeps notes short and phrases brief", () => {
    for (const phrase of phrases.flat()) {
      for (const note of phrase) {
        expect(note.duration).toBeGreaterThanOrEqual(0.015);
        expect(note.duration).toBeLessThanOrEqual(0.45);
        expect(note.start).toBeGreaterThanOrEqual(0);
      }
      const last = phrase.at(-1)!;
      expect(last.start + last.duration).toBeLessThan(2.5);
      for (let index = 1; index < phrase.length; index++) expect(phrase[index]!.start).toBeGreaterThanOrEqual(phrase[index - 1]!.start + phrase[index - 1]!.duration);
    }
  });
});
