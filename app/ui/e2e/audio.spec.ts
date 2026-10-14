import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, start } from './app';

// The audio track, which the plan's ninth phase asks for: a recording imported, the bars
// fitted to it by dragging their lines on its waveform, played along with the score, and
// saved with it.

/** A WAV file of `seconds` of clicks, two a second, at 44.1 kHz. */
function clicks(seconds: number): Buffer {
  const rate = 44_100;
  const frames = seconds * rate;
  const bytes = Buffer.alloc(44 + frames * 2);
  bytes.write('RIFF', 0, 'latin1');
  bytes.writeUInt32LE(36 + frames * 2, 4);
  bytes.write('WAVEfmt ', 8, 'latin1');
  bytes.writeUInt32LE(16, 16);
  bytes.writeUInt16LE(1, 20);
  bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(rate, 24);
  bytes.writeUInt32LE(rate * 2, 28);
  bytes.writeUInt16LE(2, 32);
  bytes.writeUInt16LE(16, 34);
  bytes.write('data', 36, 'latin1');
  bytes.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++) {
    const sinceClick = i % (rate / 2);
    const sample = sinceClick < 800 ? Math.sin(sinceClick / 3) * Math.exp(-sinceClick / 200) * 0.8 : 0;
    bytes.writeInt16LE(Math.round(sample * 32_767), 44 + i * 2);
  }
  return bytes;
}

const panel = (page: Page) => page.getByRole('region', { name: 'Audio track' });
const cursorStart = (page: Page) => panel(page).locator('[data-cursor-start]');

async function addRecording(page: Page, name: string): Promise<void> {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: 'Track', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Audio track…' }).click();
  await (await chooser).setFiles({ name, mimeType: 'audio/wav', buffer: clicks(8) });
}

/** Drags on the waveform from one time to another, in seconds, at the zoom it opens at. */
async function drag(page: Page, from: number, to: number): Promise<void> {
  const box = (await panel(page).locator('canvas').boundingBox())!;
  const x = (seconds: number) => box.x + 20 + seconds * 40;
  const y = box.y + box.height / 2;
  await page.mouse.move(x(from), y);
  await page.mouse.down();
  await page.mouse.move(x(to), y, { steps: 5 });
  await page.mouse.up();
}

test('bars are fitted to a recording, which plays along and is saved with the score', async ({ page }, testInfo) => {
  await start(page);
  // Three bars of quarter notes at 120: two seconds each as written.
  for (let note = 0; note < 12; note++) {
    await fret(page, note % 5);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
  await addRecording(page, 'clicks.wav');
  await expect(panel(page)).toContainText('clicks.wav');
  await expect(page.getByRole('status')).toHaveText('Added an audio track of clicks.wav');
  // The first bar at the start of the recording, and the waveform drawn.
  await expect(cursorStart(page)).toHaveText('Bar 1: 0:00.000, held');
  await expect
    .poll(() =>
      panel(page)
        .locator('canvas')
        .evaluate((c: HTMLCanvasElement) => {
          const data = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
          let ink = 0;
          for (let i = 3; i < data.length; i += 4) if (data[i] > 0) ink++;
          return ink;
        }),
    )
    .toBeGreaterThan(500);

  // Bar 2 dragged from two seconds to three: it is held there, and bar 3 follows it as
  // fast as the first two bars now go, a second and a half a bar.
  await drag(page, 2, 3);
  await keys(page, 'Control+Home', 'Control+ArrowRight');
  await expect(cursorStart(page)).toHaveText('Bar 2: 0:03.000, held');
  await keys(page, 'Control+ArrowRight');
  await expect(cursorStart(page)).toHaveText('Bar 3: 0:06.000');
  // Undone, as written again.
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await keys(page, 'Control+Home', 'Control+ArrowRight', 'Control+ArrowRight');
  await expect(cursorStart(page)).toHaveText('Bar 3: 0:04.000');
  await page.getByRole('button', { name: 'Redo', exact: true }).click();

  // Played, the recording is followed along, and a bar can be held to where it has got.
  await keys(page, 'Control+Home');
  await keys(page, 'Space');
  await expect(panel(page).getByRole('button', { name: /^Bar \d starts here$/ })).toBeVisible();
  await keys(page, 'Space');
  await expect(panel(page).getByRole('button', { name: /starts here/ })).toBeHidden();

  // Muted from its own switch.
  const mute = panel(page).getByRole('button', { name: 'Mute the recording' });
  await mute.click();
  await expect(mute).toHaveAttribute('aria-pressed', 'true');

  // Saved and opened again, the recording comes with it.
  const download = page.waitForEvent('download');
  await keys(page, 'Control+s');
  const saved = testInfo.outputPath('with recording.gtrtab');
  await (await download).saveAs(saved);
  await keys(page, 'Control+n');
  await expect(panel(page)).toBeHidden();
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(saved);
  await expect(panel(page)).toContainText('clicks.wav');
  await expect(panel(page).getByRole('alert')).toHaveCount(0);
  await keys(page, 'Control+ArrowRight');
  await expect(cursorStart(page)).toHaveText('Bar 2: 0:03.000, held');
  await expect(panel(page).getByRole('button', { name: 'Mute the recording' })).toHaveAttribute('aria-pressed', 'true');

  // Taken away, and back with undo.
  await panel(page).getByRole('button', { name: 'Remove the audio track' }).click();
  await expect(panel(page)).toBeHidden();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(panel(page)).toContainText('clicks.wav');
});

/** How loud a WAV file the app wrote is between two times, in seconds, from 0 to 1. */
function level(bytes: Buffer, from: number, to: number, minus?: Buffer): number {
  let sum = 0;
  const frames = Math.round((to - from) * 44_100);
  for (let frame = Math.round(from * 44_100), n = 0; n < frames; frame++, n++) {
    const at = 44 + frame * 4;
    const sample = bytes.readInt16LE(at) - (minus ? minus.readInt16LE(at) : 0);
    sum += (sample / 32_768) ** 2;
  }
  return Math.sqrt(sum / frames);
}

test('the recording is in the audio exported, unless it is muted, and in the work recovered', async ({ page }, testInfo) => {
  await start(page);
  for (let note = 0; note < 4; note++) {
    await fret(page, 5);
    await keys(page, 'ArrowRight');
  }
  await addRecording(page, 'clicks.wav');
  await expect(panel(page)).toContainText('clicks.wav');

  /** Exports the score's sound and reads it back. */
  const exported = async (name: string) => {
    const download = page.waitForEvent('download');
    await page.getByRole('menuitem', { name: 'File', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Audio (WAV)…', exact: true }).click();
    const saved = testInfo.outputPath(name);
    await (await download).saveAs(saved);
    return readFileSync(saved);
  };
  const withRecording = await exported('with.wav');
  await panel(page).getByRole('button', { name: 'Mute the recording' }).click();
  const without = await exported('without.wav');
  expect(withRecording.length).toBe(without.length);
  // What the two differ by is the clicks, two a second, and nothing between them.
  for (const click of [0, 0.5, 1, 1.5]) {
    expect(level(withRecording, click, click + 0.02, without)).toBeGreaterThan(0.05);
    expect(level(withRecording, click + 0.1, click + 0.4, without)).toBeLessThan(0.001);
  }

  // The work is kept with its recording in case the app closes, and comes back with it.
  await panel(page).getByRole('button', { name: 'Mute the recording' }).click();
  // Autosave is not instant.
  await page.waitForTimeout(2_500);
  await page.reload();
  await expect(page.getByRole('status')).toContainText('Recovered unsaved changes');
  await expect(panel(page)).toContainText('clicks.wav');
  await expect(panel(page).getByRole('alert')).toHaveCount(0);
  const recovered = await exported('recovered.wav');
  expect(level(recovered, 0, 0.02, without)).toBeGreaterThan(0.05);
});
