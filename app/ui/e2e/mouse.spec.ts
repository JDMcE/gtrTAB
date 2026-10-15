import { expect, test, type Page } from '@playwright/test';
import { fret, keys, start } from './app';

// Selecting with the mouse: dragging across beats, double and triple clicks for bars and
// lines, Shift with a click, and dragging along a track's bars in the tracks pane.

const score = (page: Page) => page.locator('[data-score]');

/** The selection as its first and last `bar,beat`, or `null` without one. */
const selection = (page: Page) => score(page).getAttribute('data-selection');

/** Where on the page the cursor is now. */
async function cursorPoint(page: Page): Promise<{ x: number; y: number }> {
  const box = (await score(page).boundingBox())!;
  const [x, y] = (await score(page).getAttribute('data-cursor'))!.split(',').map(Number);
  return { x: box.x + x, y: box.y + y };
}

/**
 * A full bar of four quarter notes and a second bar of three, and where on the page the
 * first beat of the first bar and the second beat of the second are.
 */
async function twoBars(page: Page) {
  await start(page);
  for (const number of [0, 2, 3, 5, 7, 5, 3]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
  const first = await cursorPoint(page);
  await keys(page, 'Control+ArrowRight', 'ArrowRight');
  const later = await cursorPoint(page);
  await keys(page, 'Control+Home');
  return { first, later };
}

test('a drag across the score selects the beats it crosses, and a click lets them go', async ({ page }) => {
  const { first, later } = await twoBars(page);
  await page.mouse.move(first.x, first.y);
  await page.mouse.down();
  await page.mouse.move(later.x, later.y, { steps: 8 });
  await page.mouse.up();
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,1');

  // A click, without moving, only puts the cursor down.
  await page.mouse.click(later.x, later.y);
  expect(await selection(page)).toBeNull();

  // Shift with a click selects from the cursor to the beat clicked.
  await page.keyboard.down('Shift');
  await page.mouse.click(first.x, first.y);
  await page.keyboard.up('Shift');
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,1');
});

test('a double click selects a bar, and a drag on from it more bars', async ({ page }) => {
  const { first, later } = await twoBars(page);
  await page.mouse.dblclick(first.x, first.y);
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,0,3');

  // Pressed a second time and held, the drag selects whole bars.
  await page.mouse.click(later.x, later.y);
  await page.mouse.move(later.x, later.y);
  await page.mouse.down();
  await page.mouse.move(first.x, first.y, { steps: 8 });
  await page.mouse.up();
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,2');

  // A triple click selects the whole line, which here is every bar.
  await page.mouse.click(first.x, first.y, { clickCount: 3 });
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,2');

  // Ctrl+A selects everything. The click first has to wait not to count as a fourth.
  await page.waitForTimeout(500);
  await page.mouse.click(first.x, first.y);
  expect(await selection(page)).toBeNull();
  await keys(page, 'Control+a');
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,2');
});

test('a drag along the bars in the tracks pane selects them', async ({ page }) => {
  await twoBars(page);
  const cells = page.getByRole('region', { name: 'Tracks' }).locator('.cells').first();
  const box = (await cells.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + 24, y);
  await page.mouse.down();
  await page.mouse.move(box.x + 8, y, { steps: 4 });
  await page.mouse.up();
  await expect(score(page)).toHaveAttribute('data-selection', '0,0,1,2');
  await expect(cells.locator('.selected')).toBeVisible();
});
