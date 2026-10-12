import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// The fretboard, keyboard and drum kit under the score, which the plan's eighth phase asks
// for: they light the notes at the cursor or sounding, and take notes when clicked.

async function showInstrument(page: Page): Promise<void> {
  await page.getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Fretboard, keyboard or drum kit' }).click();
}

async function addTrack(page: Page, instrument: string): Promise<void> {
  await page.getByRole('button', { name: 'Add a track…' }).click();
  await page.getByRole('dialog').getByLabel('Instrument').selectOption({ label: instrument });
  await page.getByRole('button', { name: 'OK', exact: true }).click();
}

/** What the automated accessibility checks find wrong with the instrument view. */
async function violations(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).include('section.instrument').analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

const fretboard = (page: Page) => page.getByRole('region', { name: 'Fretboard' });

test('frets clicked go into the beat at the cursor, and out again', async ({ page }) => {
  await start(page);
  await showInstrument(page);
  const board = fretboard(page);
  await expect(board).toBeVisible();
  expect(await violations(page)).toEqual([]);

  // A chord of two notes, put on the neck a fret at a time.
  await board.getByRole('button', { name: 'String 2, fret 1, C' }).click();
  await expect(position(page)).toContainText('string 2, fret 1');
  await board.getByRole('button', { name: 'String 5, fret 3, C' }).click();
  await expect(position(page)).toContainText('chord of 2 notes');
  await expect(board.getByRole('button', { name: 'String 2, fret 1, C' })).toHaveAttribute('aria-pressed', 'true');

  // Another fret on the same string moves the note; the same fret takes it out.
  await board.getByRole('button', { name: 'String 5, fret 2, B' }).click();
  await expect(board.getByRole('button', { name: 'String 5, fret 3, C' })).toHaveAttribute('aria-pressed', 'false');
  await board.getByRole('button', { name: 'String 5, fret 2, B' }).click();
  await expect(position(page)).toContainText('note,');
  await expect(board.locator('[aria-pressed="true"]')).toHaveCount(1);

  // It follows the cursor to the next beat, which is empty.
  await keys(page, 'ArrowRight');
  await expect(board.locator('[aria-pressed="true"]')).toHaveCount(0);
  await keys(page, 'ArrowLeft');
  await expect(board.locator('[aria-pressed="true"]')).toHaveCount(1);

  await board.getByRole('button', { name: 'Close the fretboard' }).click();
  await expect(board).toBeHidden();
});

test('the fretboard lights the notes as they sound', async ({ page }) => {
  await start(page);
  for (const number of [3, 5, 7, 8]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
  await showInstrument(page);
  // The notes were typed on the lowest string, where the cursor starts.
  const lit = (n: number) => fretboard(page).getByRole('button', { name: new RegExp(`^String 6, fret ${n},`) });
  await keys(page, 'Space');
  for (const n of [5, 7, 8]) await expect(lit(n)).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('button', { name: 'Stop' })).toBeHidden({ timeout: 4000 });
});

test('a piano is played on a keyboard and a drum kit on its pads', async ({ page }) => {
  await start(page);
  await showInstrument(page);
  await addTrack(page, 'Piano');
  const keyboard = page.getByRole('region', { name: 'Keyboard' });
  await expect(keyboard).toBeVisible();
  await keyboard.getByRole('button', { name: 'C4', exact: true }).click();
  await keyboard.getByRole('button', { name: 'E4', exact: true }).click();
  await expect(position(page)).toContainText('chord of 2 notes');
  await keyboard.getByRole('button', { name: 'C4', exact: true }).click();
  await expect(position(page)).toContainText('quarter note, at C 4, no note');
  await expect(keyboard.getByRole('button', { name: 'E4', exact: true })).toHaveAttribute('aria-pressed', 'true');
  expect(await violations(page)).toEqual([]);

  await addTrack(page, 'Drums');
  const kit = page.getByRole('region', { name: 'Drum kit' });
  await kit.getByRole('button', { name: 'Bass drum', exact: true }).click();
  await kit.getByRole('button', { name: 'Closed hi-hat', exact: true }).click();
  await expect(position(page)).toContainText('chord of 2 notes');
  await expect(position(page)).toContainText('Closed hi-hat');
  await kit.getByRole('button', { name: 'Bass drum', exact: true }).click();
  await expect(position(page)).toContainText('note,');
  await expect(kit.getByRole('button', { name: 'Closed hi-hat', exact: true })).toHaveAttribute('aria-pressed', 'true');
  expect(await violations(page)).toEqual([]);
  await expect(page.locator('footer.failed')).toHaveCount(0);
});
