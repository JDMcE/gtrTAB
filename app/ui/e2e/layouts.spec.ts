import { expect, test, type Page } from '@playwright/test';
import { keys, position, start } from './app';

// The layouts of the score on the screen, which the plan's tenth phase asks for: pages one
// under another or side by side, parchment wrapped to the window, and one line of every bar.

const scroller = (page: Page) => page.locator('[data-score]');
const scroll = (page: Page) =>
  scroller(page).evaluate((el) => ({ left: el.scrollLeft, top: el.scrollTop, width: el.scrollWidth, height: el.scrollHeight }));

async function chooseLayout(page: Page, name: string): Promise<void> {
  await page.getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Layout' }).click();
  await page.getByRole('menuitemcheckbox', { name }).click();
}

/** A score of `count` empty bars, long enough for several pages. */
async function bars(page: Page, count: number): Promise<void> {
  await start(page);
  for (let i = 1; i < count; i++) await keys(page, 'Control+Insert');
  await keys(page, 'Control+Home');
  await expect(position(page)).toContainText('Bar 1,');
}

test('pages side by side scroll across, and the cursor and a click find their bars on any page', async ({ page }) => {
  test.slow();
  await bars(page, 240);
  await chooseLayout(page, 'Pages side by side');
  const across = await scroll(page);
  expect(across.width).toBeGreaterThan(2 * 794);
  expect(across.height).toBeLessThan(1300);

  // To the last bar, which is on the last page: the view follows it across.
  await keys(page, 'Control+End');
  await expect(position(page)).toContainText('Bar 240,');
  await expect.poll(async () => (await scroll(page)).left).toBeGreaterThan(794);

  // A click near the top left of the page in view is on a bar of that page, not the first.
  const box = (await scroller(page).boundingBox())!;
  const { left } = await scroll(page);
  const pageStart = Math.ceil((left - 24) / (794 + 24)) * (794 + 24) + 24 - left;
  await page.mouse.click(box.x + pageStart + 120, box.y + 80);
  await expect(position(page)).not.toContainText('Bar 240,');
  const clicked = Number(/Bar (\d+)/.exec((await position(page).textContent()) ?? '')![1]);
  expect(clicked).toBeGreaterThan(40);

  // One under another, the same score scrolls down instead.
  await chooseLayout(page, 'Pages, one under another');
  const down = await scroll(page);
  expect(down.height).toBeGreaterThan(2 * 1123);
  expect(down.width).toBeLessThanOrEqual(box.width + 1);
  // Back at the first bar, the view is back at the first page.
  await keys(page, 'Control+Home');
  await expect.poll(async () => (await scroll(page)).top).toBeLessThan(300);
});

test('one line of every bar scrolls across, and the choice is kept', async ({ page }) => {
  await bars(page, 60);
  await chooseLayout(page, 'Screen: every bar on one line');
  const line = await scroll(page);
  expect(line.width).toBeGreaterThan(3000);
  await keys(page, 'Control+End');
  await expect.poll(async () => (await scroll(page)).left).toBeGreaterThan(2000);
  expect((await scroll(page)).top).toBe(0);

  // Kept for next time.
  await page.reload();
  await expect(position(page)).toContainText('Bar');
  await page.getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Layout' }).click();
  await expect(page.getByRole('menuitemcheckbox', { name: 'Screen: every bar on one line' })).toHaveAttribute('aria-checked', 'true');
  await keys(page, 'Escape', 'Escape', 'Escape');
  await chooseLayout(page, 'Parchment: one sheet as wide as the window');
  await expect.poll(async () => (await scroll(page)).width).toBeLessThan(1400);
});
