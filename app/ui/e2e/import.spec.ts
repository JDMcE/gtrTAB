import { expect, test } from '@playwright/test';
import { keys, place, position, start } from './app';

// Guitar Pro files of every version open in the app. What each holds is the importer's
// own tests' business; here it is that the file gets from the dialog to the score.

const corpus = new URL('../../../tests/corpus/alphatab/', import.meta.url).pathname;

for (const file of [
  'guitarpro3/dead.gp3',
  'guitarpro4/dead.gp4',
  'guitarpro5/dead.gp5',
  'guitarpro6/dead.gpx',
  'guitarpro7/dead.gp',
]) {
  test(`${file} opens`, async ({ page }) => {
    await start(page);
    const chooser = page.waitForEvent('filechooser');
    await keys(page, 'Control+o');
    await (await chooser).setFiles(corpus + file);
    // Four dead notes, one on each of the four low strings; the cursor starts on the first.
    await expect(position(page)).toHaveText('Bar 1, beat 1 of 4, quarter note, string 6, fret 1, F 2, dead note.');
    await keys(page, 'ArrowRight', 'ArrowUp');
    await expect(position(page)).toHaveText('Bar 1, beat 2 of 4, quarter note, string 5, fret 2, B 2, dead note.');
    // It is a new score with the file's name, to be saved in gtrTab's own format.
    await expect(page.locator('.file')).toHaveText('dead.gtrtab');
    await expect(place(page)).toContainText('Bar 1 of');
  });
}

test('chords and lyrics come in with the notes', async ({ page }) => {
  await start(page);
  let chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(corpus + 'guitarpro6/chords.gpx');
  await expect(position(page)).toContainText('quarter chord of 5 notes');
  await page.getByRole('button', { name: 'Inspector' }).click();
  await expect(page.locator('.inspector')).toContainText('C');

  // An imported score is unsaved work, so opening another asks first.
  await keys(page, 'Escape');
  page.once('dialog', (dialog) => void dialog.accept());
  chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(corpus + 'guitarpro7/beat-lyrics.gp');
  await expect(position(page)).toContainText('Bar 1, beat 1 of 4');
  await expect(page.locator('.inspector')).toContainText('This');
});
