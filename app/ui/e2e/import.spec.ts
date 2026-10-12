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

// The other formats: each gets from the dialog to the score, known for what it is.

const others = new URL('../../../tests/corpus/', import.meta.url).pathname;

test('a MusicXML file with tablature opens with its strings and frets', async ({ page }) => {
  await start(page);
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(others + 'alphatab/musicxml-testsuite/71e-TabStaves.xml');
  await expect(page.getByRole('status')).toContainText('Imported 71e-TabStaves.xml.');
  await expect(page.locator('.file')).toHaveText('71e-TabStaves.gtrtab');
  // The cursor starts on the lowest string; the first note is two strings up.
  await keys(page, 'ArrowUp', 'ArrowUp');
  await expect(position(page)).toHaveText('Bar 1, beat 1 of 4, quarter note, string 4, fret 5, G 3.');
});

test('a MIDI file opens as the notes it plays', async ({ page }) => {
  await start(page);
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(others + 'mutopia/Bach_Prelude_BWV999.mid');
  // Sixteenths in three, over a bass on D that rings on under them.
  await keys(page, 'ArrowUp', 'ArrowUp');
  await expect(position(page)).toHaveText(
    'Bar 1, beat 1 of 12, sixteenth note, string 4, fret 0, D 3, let ring.',
  );
  await expect(place(page)).toContainText('Bar 1 of 43');
});

test('a tab typed as text opens with lengths judged from its spacing', async ({ page }) => {
  await start(page);
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(others + 'ascii/ode-to-joy.txt');
  await keys(page, 'ArrowUp', 'ArrowUp', 'ArrowUp', 'ArrowUp', 'ArrowUp');
  await expect(position(page)).toHaveText('Bar 1, beat 1 of 4, quarter note, string 1, fret 0, E 4.');
  await keys(page, 'Control+End');
  await expect(position(page)).toContainText('Bar 4,');
  await expect(page.locator('.file')).toHaveText('ode-to-joy.gtrtab');
});

test('a file that is nothing it knows is refused with a reason', async ({ page }) => {
  await start(page);
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles({ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('Just some words.') });
  await expect(page.getByRole('status')).toContainText('Could not import notes.txt');
  await expect(position(page)).toHaveText(/^Bar 1, empty/);
});
