import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { fret, keys, start } from './app';

test('the score is exported as a WAV file that has its notes in it', async ({ page }, testInfo) => {
  await start(page);
  // Four quarter notes at 120: two seconds of music.
  for (const number of [0, 3, 5, 7]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  const download = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Export audio (WAV)…' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('untitled.wav');
  await expect(page.getByRole('status')).toHaveText('Exported untitled.wav');
  const saved = testInfo.outputPath('untitled.wav');
  await file.saveAs(saved);

  const bytes = readFileSync(saved);
  expect(bytes.toString('latin1', 0, 4)).toBe('RIFF');
  expect(bytes.toString('latin1', 8, 16)).toBe('WAVEfmt ');
  // Plain samples, two channels, 44.1 kHz, sixteen bits.
  expect([bytes.readUInt16LE(20), bytes.readUInt16LE(22), bytes.readUInt32LE(24), bytes.readUInt16LE(34)]).toEqual([
    1, 2, 44_100, 16,
  ]);
  const frames = bytes.readUInt32LE(40) / 4;
  expect(bytes.length).toBe(44 + frames * 4);
  // The two seconds written, and the last note left to die away, but not for ever.
  expect(frames).toBeGreaterThanOrEqual(2 * 44_100);
  expect(frames).toBeLessThan(12 * 44_100);

  /** How loud a stretch of the file is, from 0 to 1. */
  const level = (from: number, to: number) => {
    let sum = 0;
    for (let frame = Math.round(from * 44_100); frame < Math.round(to * 44_100); frame++) {
      sum += (bytes.readInt16LE(44 + frame * 4) / 32_768) ** 2;
    }
    return Math.sqrt(sum / ((to - from) * 44_100));
  };
  // Each of the four notes can be heard, and none of it is cut off at full scale.
  for (const second of [0, 0.5, 1, 1.5]) expect(level(second, second + 0.5)).toBeGreaterThan(0.005);
  let peak = 0;
  for (let at = 44; at < bytes.length; at += 2) peak = Math.max(peak, Math.abs(bytes.readInt16LE(at)));
  expect(peak).toBeLessThan(32_767);
  // The export leaves the score as it was, saved or not, and can be asked for again.
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'Export audio (WAV)…' })).toBeEnabled();
});
