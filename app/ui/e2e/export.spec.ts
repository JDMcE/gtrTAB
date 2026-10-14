import { readFileSync } from 'node:fs';
import { expect, test, type Page, type TestInfo } from '@playwright/test';
import { fret, keys, position, start } from './app';

/** Chooses an entry of the File menu's Export submenu. */
async function exportAs(page: Page, item: string): Promise<void> {
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
  await page.getByRole('menuitem', { name: item, exact: true }).click();
}

test('the score is exported as a WAV file that has its notes in it', async ({ page }, testInfo) => {
  await start(page);
  // Four quarter notes at 120: two seconds of music.
  for (const number of [0, 3, 5, 7]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  const download = page.waitForEvent('download');
  await exportAs(page, 'Audio (WAV)…');
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
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'Audio (WAV)…' })).toBeEnabled();
});

/** Writes a short riff: four quarter notes up the first string. */
async function riff(page: Page): Promise<void> {
  await start(page);
  for (const number of [0, 3, 5, 7]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
}

/** Exports the score in one of the ways the Export menu offers, and gives the file's bytes. */
async function exported(page: Page, testInfo: TestInfo, item: string, name: string): Promise<Buffer> {
  const download = page.waitForEvent('download');
  await exportAs(page, item);
  const file = await download;
  expect(file.suggestedFilename()).toBe(name);
  await expect(page.getByRole('status')).toContainText(`Exported ${name}`);
  const saved = testInfo.outputPath(name);
  await file.saveAs(saved);
  return readFileSync(saved);
}

test('the score is exported in the formats of other programs', async ({ page }, testInfo) => {
  await riff(page);
  // A `.gp` file is a zip archive with the score in it.
  const gp = await exported(page, testInfo, 'Guitar Pro (.gp)…', 'untitled.gp');
  expect(gp.toString('latin1', 0, 2)).toBe('PK');
  expect(gp.includes('Content/score.gpif')).toBe(true);
  // MusicXML says the notes with their strings and frets.
  const xml = (await exported(page, testInfo, 'MusicXML…', 'untitled.musicxml')).toString('utf8');
  expect(xml).toContain('<score-partwise version="4.0">');
  expect(xml.match(/<fret>\d+<\/fret>/g)).toEqual(['<fret>0</fret>', '<fret>3</fret>', '<fret>5</fret>', '<fret>7</fret>']);
  // MIDI: a header, and four notes started.
  const midi = await exported(page, testInfo, 'MIDI…', 'untitled.mid');
  expect(midi.toString('latin1', 0, 4)).toBe('MThd');
  expect([...midi].filter((byte, at) => byte === 0x90 && midi[at + 2] === 80).length).toBe(4);
  // Text: the lowest string's line with the four frets on it, quarters over them.
  const tab = (await exported(page, testInfo, 'Text tab…', 'untitled.txt')).toString('utf8');
  expect(tab).toContain('   Q   Q   Q   Q');
  expect(tab).toContain('e|-----------------|');
  expect(tab).toContain('E|-0---3---5---7---|');
  await expect(page.getByRole('status')).toHaveText('Exported untitled.txt');
});

test('what a format has no place for is said when the score is exported', async ({ page }, testInfo) => {
  await riff(page);
  await keys(page, 'Home');
  await page.getByRole('menuitem', { name: 'Beat', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Dynamic', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Fortissimo (ff)' }).click();
  await exported(page, testInfo, 'Text tab…', 'untitled.txt');
  await expect(page.getByRole('status')).toHaveText('Exported untitled.txt. Not exported: dynamics.');
});

test('the score is exported as a PDF and as pictures', async ({ page }, testInfo) => {
  await riff(page);
  const pdf = await exported(page, testInfo, 'PDF…', 'untitled.pdf');
  expect(pdf.toString('latin1', 0, 8)).toBe('%PDF-1.4');
  expect(pdf.toString('latin1').trimEnd().endsWith('%%EOF')).toBe(true);
  expect(pdf.toString('latin1').match(/\/Type \/Page /g)?.length).toBe(1);

  const png = await exported(page, testInfo, 'Picture (PNG)…', 'untitled.png');
  expect(png.toString('latin1', 1, 4)).toBe('PNG');
  // Two pixels to each of the thousand the score is laid out across.
  expect(png.readUInt32BE(16)).toBe(2000);
  expect(png.readUInt32BE(20)).toBeGreaterThan(400);

  // The SVG is drawn in outlines: no text for a font to be found for.
  const svg = (await exported(page, testInfo, 'Picture (SVG)…', 'untitled.svg')).toString('utf8');
  expect(svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg" width="1000"')).toBe(true);
  expect(svg).not.toContain('<text');
  expect(svg.match(/<path d="M/g)?.length).toBeGreaterThan(10);
  // The picture shows what the browser makes of it: mostly paper, with ink on it.
  const shown = await page.evaluate(async (markup) => {
    const image = new Image();
    image.src = URL.createObjectURL(new Blob([markup], { type: 'image/svg+xml' }));
    await image.decode();
    const canvas = document.createElement('canvas');
    [canvas.width, canvas.height] = [image.width, image.height];
    const context = canvas.getContext('2d')!;
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let [paper, ink] = [0, 0];
    for (let at = 0; at < pixels.length; at += 4) {
      if (pixels[at] > 250) paper++;
      else if (pixels[at] < 64) ink++;
    }
    return { paper: paper / (pixels.length / 4), ink: ink / (pixels.length / 4) };
  }, svg);
  expect(shown.paper).toBeGreaterThan(0.8);
  expect(shown.ink).toBeGreaterThan(0.002);
});

test('the score is printed a page to a sheet', async ({ page }) => {
  // The print dialog cannot be driven, so in its place stands something that notes
  // what the window holds when it is asked to print.
  await page.addInitScript(() => {
    window.print = () => {
      const pages = document.querySelectorAll('.print-root > svg');
      const first = pages[0]?.getAttribute('viewBox') ?? '';
      (window as unknown as { printed: unknown }).printed = { pages: pages.length, first };
      window.dispatchEvent(new Event('afterprint'));
    };
  });
  await riff(page);
  await keys(page, 'Control+p');
  await expect.poll(() => page.evaluate(() => (window as unknown as { printed?: unknown }).printed)).toBeTruthy();
  const printed = await page.evaluate(() => (window as unknown as { printed: { pages: number; first: string } }).printed);
  // One page, the size of the paper usual where the browser says it is.
  expect(printed.pages).toBe(1);
  expect(['0 0 794 1123', '0 0 816 1056']).toContain(printed.first);
  // Once printed, the pages are taken out of the window again.
  await expect(page.locator('.print-root')).toHaveCount(0);
  await expect(position(page)).toContainText('Bar 2, empty');
});
