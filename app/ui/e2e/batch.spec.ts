import { readFileSync } from 'node:fs';
import { inflateRawSync } from 'node:zlib';
import { expect, test, type Page } from '@playwright/test';
import { start } from './app';

const corpus = new URL('../../../tests/corpus/', import.meta.url).pathname;

type File = { name: string; mimeType: string; buffer: Buffer };

/** A file of the corpus, read in, so that it can be given alongside one made up. */
const read = (path: string): File => ({
  name: path.split('/').pop()!,
  mimeType: 'application/octet-stream',
  buffer: readFileSync(corpus + path),
});

/** Opens File > Batch convert… and adds the files given. */
async function batch(page: Page, files: File[]) {
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Batch convert…', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Batch convert' });
  const chooser = page.waitForEvent('filechooser');
  await dialog.getByRole('button', { name: 'Add scores…' }).click();
  await (await chooser).setFiles(files);
  return dialog;
}

/** The files in a zip archive, by name: enough of a reader for what the app writes. */
function unzip(bytes: Buffer): Map<string, Buffer> {
  const files = new Map<string, Buffer>();
  const end = bytes.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  let at = bytes.readUInt32LE(end + 16);
  for (let n = bytes.readUInt16LE(end + 10); n > 0; n--) {
    const method = bytes.readUInt16LE(at + 10);
    const size = bytes.readUInt32LE(at + 20);
    const nameLength = bytes.readUInt16LE(at + 28);
    const skip = nameLength + bytes.readUInt16LE(at + 30) + bytes.readUInt16LE(at + 32);
    const name = bytes.toString('utf8', at + 46, at + 46 + nameLength);
    const local = bytes.readUInt32LE(at + 42);
    const data = local + 30 + bytes.readUInt16LE(local + 26) + bytes.readUInt16LE(local + 28);
    const stored = bytes.subarray(data, data + size);
    files.set(name, method === 0 ? stored : inflateRawSync(stored));
    at += 46 + skip;
  }
  return files;
}

test('scores of every format are converted together and downloaded as one zip file', async ({ page }, testInfo) => {
  await start(page);
  const dialog = await batch(page, [
    read('alphatab/guitarpro5/dead.gp5'),
    read('alphatab/guitarpro7/dead.gp'),
    read('mutopia/Bach_Prelude_BWV999.mid'),
    read('ascii/ode-to-joy.txt'),
    { name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('Just some words.') },
  ]);
  const scores = dialog.getByRole('list', { name: 'Scores' });
  await expect(scores.getByRole('listitem')).toHaveCount(5);
  // A file added twice is converted once, and one can be taken out again.
  const chooser = page.waitForEvent('filechooser');
  await dialog.getByRole('button', { name: 'Add scores…' }).click();
  await (await chooser).setFiles([read('ascii/ode-to-joy.txt'), read('ascii/blues-lick.txt')]);
  await expect(scores.getByRole('listitem')).toHaveCount(6);
  await dialog.getByRole('button', { name: 'Take blues-lick.txt out' }).click();
  await expect(scores.getByRole('listitem')).toHaveCount(5);

  await dialog.getByLabel('Convert to').selectOption({ label: 'MusicXML' });
  const download = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Convert', exact: true }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('converted.zip');

  const results = dialog.getByRole('list', { name: 'Converted' });
  await expect(results.getByRole('listitem')).toHaveCount(5);
  await expect(results).toContainText('dead.gp5 → dead.musicxml');
  // Two scores of the same name are not written over each other.
  await expect(results).toContainText('dead.gp → dead (2).musicxml');
  await expect(results).toContainText('Bach_Prelude_BWV999.mid → Bach_Prelude_BWV999.musicxml');
  await expect(results.getByRole('listitem').last()).toContainText('notes.txt Failed: Could not import notes.txt: not a file of a kind');
  await expect(dialog.getByRole('status')).toContainText(
    '4 scores converted. 1 could not be. Downloaded as converted.zip.',
  );

  const saved = testInfo.outputPath('converted.zip');
  await file.saveAs(saved);
  const files = unzip(readFileSync(saved));
  expect([...files.keys()]).toEqual([
    'dead.musicxml',
    'dead (2).musicxml',
    'Bach_Prelude_BWV999.musicxml',
    'ode-to-joy.musicxml',
  ]);
  for (const contents of files.values()) expect(contents.toString('utf8')).toContain('<score-partwise');

  // The score being edited is left as it was.
  await dialog.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.locator('.file')).toHaveText('untitled.gtrtab');
});

test('scores are converted to pictures, and the format chosen is kept for next time', async ({ page }, testInfo) => {
  await start(page);
  let dialog = await batch(page, [read('ascii/ode-to-joy.txt'), read('ascii/rock-beat.txt')]);
  await dialog.getByLabel('Convert to').selectOption({ label: 'PDF' });
  const download = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Convert', exact: true }).click();
  const saved = testInfo.outputPath('converted.zip');
  await (await download).saveAs(saved);
  const files = unzip(readFileSync(saved));
  expect([...files.keys()]).toEqual(['ode-to-joy.pdf', 'rock-beat.pdf']);
  for (const contents of files.values()) expect(contents.toString('latin1', 0, 5)).toBe('%PDF-');

  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  dialog = await batch(page, [read('ascii/ode-to-joy.txt')]);
  await expect(dialog.getByLabel('Convert to')).toHaveValue('pdf');
});
