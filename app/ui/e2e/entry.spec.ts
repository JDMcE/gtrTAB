import { expect, test } from '@playwright/test';
import { fret, keys, place, position, start } from './app';

// The core of the editor, worked from the keyboard alone as the plan's second phase asks:
// write a riff, change it, hear it, save it and open it again.

test('a riff is written from the keyboard', async ({ page }) => {
  await start(page);
  await expect(position(page)).toHaveText('Bar 1, empty, string 6, no note.');

  // Two quarter notes on the low string.
  await fret(page, 3);
  await expect(position(page)).toHaveText('Bar 1, beat 1 of 1, quarter note, string 6, fret 3, G 2.');
  await keys(page, 'ArrowRight');
  await expect(position(page)).toHaveText('Bar 1, after the last of 1 beats, empty, string 6, no note.');
  await fret(page, 5);

  // Eighth notes next, a string up, the first hammered on to the second.
  await keys(page, 'ArrowRight', '+', 'ArrowUp');
  await fret(page, 2);
  await keys(page, 'h');
  await expect(position(page)).toHaveText(
    'Bar 1, beat 3 of 3, eighth note, string 5, fret 2, B 2, hammer-on or pull-off.',
  );
  await keys(page, 'ArrowRight');
  await fret(page, 3);

  // A chord: a second note on the same beat, and a fret of two digits.
  await keys(page, 'ArrowRight', '-');
  await fret(page, 12);
  await keys(page, 'ArrowUp');
  await fret(page, 10);
  await expect(position(page)).toHaveText(
    'Bar 1, beat 5 of 5, quarter chord of 2 notes, string 4, fret 10, C 4.',
  );

  // The bar is full, so the next step is into the bar after it, which the score grew.
  await keys(page, 'ArrowRight');
  await expect(place(page)).toContainText('Bar 2 of 2');
  await expect(position(page)).toHaveText(/^Bar 2, empty/);

  // A rest, then a dotted note; and back to the start of the piece.
  await keys(page, 'r');
  await expect(position(page)).toHaveText('Bar 2, beat 1 of 1, quarter rest, string 4, no note.');
  await keys(page, 'ArrowRight', '.');
  await fret(page, 7);
  await expect(position(page)).toContainText('dotted quarter note, string 4, fret 7, A 3.');
  await keys(page, 'Control+Home');
  await expect(position(page)).toContainText('Bar 1, beat 1 of 5');
});

test('edits are undone, redone and removed', async ({ page }) => {
  await start(page);
  await fret(page, 5);
  await keys(page, 'p', 'v');
  await expect(position(page)).toContainText('fret 5, A 2, vibrato, palm mute.');

  await keys(page, 'Control+z');
  await expect(position(page)).toContainText('fret 5, A 2, palm mute.');
  await keys(page, 'Control+z', 'Control+z');
  await expect(position(page)).toHaveText('Bar 1, empty, string 6, no note.');
  await keys(page, 'Control+y', 'Control+y');
  await expect(position(page)).toContainText('fret 5, A 2, palm mute.');

  // Delete takes the note away and leaves a rest; again, and the beat goes too.
  await keys(page, 'Delete');
  await expect(position(page)).toContainText('quarter rest');
  await keys(page, 'Delete');
  await expect(position(page)).toHaveText('Bar 1, empty, string 6, no note.');
});

test('beats and bars are copied and pasted', async ({ page }) => {
  await start(page);
  for (const number of [1, 4, 6]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  // With nothing selected the beat under the cursor is copied: the second, pasted at the end.
  await keys(page, 'ArrowLeft', 'ArrowLeft', 'Control+c', 'ArrowRight', 'ArrowRight', 'Control+v');
  await keys(page, 'Control+Home');
  for (const [beat, number] of [[1, 1], [2, 4], [3, 6], [4, 4]]) {
    await expect(position(page)).toContainText(`beat ${beat} of 4, quarter note, string 6, fret ${number},`);
    await keys(page, 'ArrowRight');
  }

  // A selection of the whole bar is copied as a bar, and pasted as a new one before the cursor's.
  await expect(place(page)).toContainText('Bar 2 of 2');
  await keys(page, 'Control+Home', 'Shift+ArrowRight', 'Shift+ArrowRight', 'Shift+ArrowRight', 'Control+c', 'Control+v');
  await expect(place(page)).toContainText('of 3');
  await keys(page, 'Control+End', 'Control+ArrowLeft');
  await expect(position(page)).toContainText('Bar 2, beat 1 of 4, quarter note, string 6, fret 1,');
});

test('a score is saved and opened again', async ({ page }, testInfo) => {
  await start(page);
  await fret(page, 7);
  await keys(page, 'b', 'ArrowRight');
  await fret(page, 9);

  const download = page.waitForEvent('download');
  await keys(page, 'Control+s');
  const name = (await download).suggestedFilename();
  expect(name).toMatch(/\.gtrtab$/);
  const saved = testInfo.outputPath(name);
  await (await download).saveAs(saved);

  // A new score has nothing in it; opening the file brings the riff back.
  await keys(page, 'Control+n');
  await expect(position(page)).toHaveText(/^Bar 1, empty/);
  const chooser = page.waitForEvent('filechooser');
  await keys(page, 'Control+o');
  await (await chooser).setFiles(saved);
  await expect(position(page)).toHaveText('Bar 1, beat 1 of 2, quarter note, string 6, fret 7, B 2, bend.');
  await keys(page, 'ArrowRight');
  await expect(position(page)).toContainText('fret 9, C sharp 3.');
});

test('a file dropped on the window is opened, after asking about unsaved work', async ({ page }, testInfo) => {
  await start(page);
  await fret(page, 4);
  const download = page.waitForEvent('download');
  await keys(page, 'Control+s');
  const saved = testInfo.outputPath('dropped.gtrtab');
  await (await download).saveAs(saved);
  const bytes = [...(await import('node:fs')).readFileSync(saved)];

  // Something else is being written, unsaved, when the file is dropped.
  await keys(page, 'ArrowRight');
  await fret(page, 11);
  const drop = () =>
    page.evaluate((data) => {
      const transfer = new DataTransfer();
      transfer.items.add(new File([new Uint8Array(data)], 'dropped.gtrtab'));
      const target = document.querySelector('[data-score]')!;
      target.dispatchEvent(new DragEvent('dragover', { dataTransfer: transfer, bubbles: true, cancelable: true }));
      target.dispatchEvent(new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }));
    }, bytes);

  // Asked, and answered no: the work stays.
  page.once('dialog', (dialog) => {
    expect(dialog.message()).toContain('Discard unsaved changes');
    void dialog.dismiss();
  });
  await drop();
  await expect(position(page)).toContainText('fret 11,');
  // Answered yes: the dropped score takes its place.
  page.once('dialog', (dialog) => void dialog.accept());
  await drop();
  await expect(position(page)).toHaveText('Bar 1, beat 1 of 1, quarter note, string 6, fret 4, A flat 2.');
});

test('unsaved work comes back after the page is closed', async ({ page }) => {
  await start(page);
  await fret(page, 8);
  // Autosave is not instant.
  await page.waitForTimeout(2500);
  await page.reload();
  await expect(page.getByRole('status')).toContainText('Recovered unsaved changes');
  await expect(position(page)).toContainText('fret 8, C 3.');
});

test('playback starts and stops from the keyboard', async ({ page }) => {
  await start(page);
  await fret(page, 0);
  await keys(page, 'ArrowRight');
  await fret(page, 3);
  await keys(page, 'Space');
  await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible();
  await keys(page, 'Space');
  await expect(page.getByRole('button', { name: 'Stop' })).toBeHidden();
  await expect(page.locator('footer.failed')).toHaveCount(0);
});
