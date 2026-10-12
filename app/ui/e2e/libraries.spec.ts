import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// The chord and scale libraries and the scale finder, which the plan's eighth phase asks for.

async function tool(page: Page, item: string): Promise<void> {
  await page.getByRole('menuitem', { name: 'Tools', exact: true }).click();
  await page.getByRole('menuitem', { name: item, exact: true }).click();
}

test('a chord is found in the library, heard, shown and put on the beat', async ({ page }) => {
  await start(page);
  await tool(page, 'Chord library…');
  const dialog = page.getByRole('dialog', { name: 'Chord library' });
  await dialog.getByLabel('Root').selectOption({ label: 'A' });
  await dialog.getByLabel('Chord').selectOption({ label: 'Minor' });
  await expect(dialog).toContainText('A C E');
  const shapes = dialog.getByRole('group', { name: 'Ways to play Am' }).getByRole('button');
  // The open shape comes first, chosen; another can be chosen instead.
  await expect(shapes.first()).toHaveAccessibleName('Am: x 0 2 2 1 0');
  await expect(shapes.first()).toHaveAttribute('aria-pressed', 'true');
  await shapes.nth(1).click();
  await expect(shapes.nth(1)).toHaveAttribute('aria-pressed', 'true');
  await shapes.first().click();
  await dialog.getByRole('button', { name: 'Hear it' }).click();

  await dialog.getByRole('button', { name: 'Put on the beat' }).click();
  await expect(dialog).toBeHidden();
  await expect(position(page)).toContainText('chord of 5 notes');
  // One edit, undone in one step.
  await keys(page, 'Control+z');
  await expect(position(page)).toContainText('empty');

  await tool(page, 'Chord library…');
  await dialog.getByLabel('Root').selectOption({ label: 'G' });
  await dialog.getByRole('button', { name: 'Show on the fretboard' }).click();
  const board = page.getByRole('region', { name: 'Fretboard' });
  await expect(board).toContainText('Marked: G');
  await board.getByRole('button', { name: 'Clear the marks' }).click();
  await expect(board).not.toContainText('Marked');
  await expect(page.locator('footer.failed')).toHaveCount(0);
});

test('a scale is drawn from the library with its notes spelled', async ({ page }) => {
  await start(page);
  await tool(page, 'Scale library…');
  const dialog = page.getByRole('dialog', { name: 'Scale library' });
  await dialog.getByLabel('Root').selectOption({ label: 'F' });
  const notes = dialog.getByRole('table');
  await expect(notes).toHaveAccessibleName('Notes of F major');
  await expect(notes.getByRole('row').first()).toHaveText(/F\s*G\s*A\s*Bb\s*C\s*D\s*E/);
  await dialog.getByRole('combobox', { name: 'Scale', exact: true }).selectOption({ label: 'Blues' });
  await expect(notes.getByRole('row').nth(1)).toHaveText(/1\s*♭3\s*4\s*♭5\s*5\s*♭7/);
  await dialog.getByRole('button', { name: 'Hear it' }).click();
  await dialog.getByRole('button', { name: 'Show on the fretboard' }).click();
  await expect(page.getByRole('region', { name: 'Fretboard' })).toContainText('Marked: F blues');
  await expect(page.locator('footer.failed')).toHaveCount(0);
});

test('the scale finder names the scale the notes come from', async ({ page }) => {
  await start(page);
  // A, C, D, E and G on the low E string, and back to A.
  for (const number of [5, 8, 10, 12, 15, 5]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  // With nothing selected, the whole track is looked at.
  await tool(page, 'Find the scale of the selection…');
  const dialog = page.getByRole('dialog', { name: 'Scale library' });
  await expect(dialog.getByRole('group', { name: 'Scales that fit the notes of the whole track' })).toBeVisible();
  const best = dialog.getByRole('radio').first();
  await expect(best).toBeChecked();
  await expect(dialog.locator('.found label').first()).toHaveText(/^\s*A minor pentatonic: every note$/);
  await expect(dialog.getByRole('table')).toHaveAccessibleName('Notes of A minor pentatonic');
  // Another scale that fits is chosen from the list.
  await dialog.getByRole('radio').nth(1).check();
  await expect(dialog.getByRole('table')).not.toHaveAccessibleName('Notes of A minor pentatonic');
  await keys(page, 'Escape');

  // Of the first two notes alone, A and C, the finder says what fits them.
  await keys(page, 'Control+Home', 'Shift+ArrowRight');
  await tool(page, 'Find the scale of the selection…');
  await expect(dialog.getByRole('group', { name: 'Scales that fit the notes of the selection' })).toBeVisible();
  await expect(dialog.locator('.found label').first()).toContainText('every note');
});
