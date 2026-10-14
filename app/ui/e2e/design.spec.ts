import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// Stylesheets and design mode, which the plan's tenth phase asks for: how the score is set
// out, and where its lines and pages break and how wide its bars are.

const score = (page: Page) => page.locator('[data-score]');
const lines = async (page: Page) => Number(await score(page).getAttribute('data-lines'));
const palette = (page: Page) => page.getByRole('complementary', { name: 'Tools' });

/** A score of `count` bars with a note in each. */
async function bars(page: Page, count: number): Promise<void> {
  await start(page);
  for (let i = 0; i < count; i++) {
    await fret(page, i % 10);
    if (i < count - 1) await keys(page, 'Control+Insert');
  }
  await keys(page, 'Control+Home');
}

test('a bar is made to start a line, and given a width, in design mode', async ({ page }) => {
  await bars(page, 8);
  const before = await lines(page);
  await page.getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: /^Design mode/ }).click();
  await expect(palette(page).getByRole('group', { name: 'Design' })).toBeVisible();

  // The first bar always starts a line.
  await expect(palette(page).getByRole('button', { name: 'Start a new line with this bar' })).toHaveAttribute('aria-disabled', 'true');
  await keys(page, 'Control+ArrowRight', 'Control+ArrowRight');
  await expect(position(page)).toContainText('Bar 3,');
  await palette(page).getByRole('button', { name: 'Start a new line with this bar' }).click();
  await expect.poll(() => lines(page)).toBe(before + 1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => lines(page)).toBe(before);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();

  // So many bars to a line, from the palette: two, then two from the break before bar 3.
  await palette(page).getByRole('button', { name: 'Bars on each line…' }).click();
  await page.getByRole('spinbutton', { name: 'Bars', exact: true }).fill('2');
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect.poll(() => lines(page)).toBe(4);

  // Dragged, the bar's right edge makes it wider; the menu can put it back.
  const edge = (await score(page).getAttribute('data-edge'))!.split(',').map(Number);
  const box = (await score(page).boundingBox())!;
  await page.mouse.move(box.x + edge[0], box.y + edge[1]);
  await expect(score(page)).toHaveClass(/resize/);
  await page.mouse.down();
  await page.mouse.move(box.x + edge[0] + 80, box.y + edge[1], { steps: 5 });
  await page.mouse.up();
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Lines and pages' }).click();
  const reset = page.getByRole('menuitem', { name: 'Bar width as it comes' });
  await expect(reset).toHaveAttribute('aria-disabled', 'false');
  await reset.click();
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Lines and pages' }).click();
  await expect(page.getByRole('menuitem', { name: 'Bar width as it comes' })).toHaveAttribute('aria-disabled', 'true');
  await keys(page, 'Escape', 'Escape', 'Escape');
});

test('the stylesheet sets out the score, is kept with it, and goes back as it came', async ({ page }) => {
  await bars(page, 12);
  const before = await lines(page);
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Stylesheet…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Stylesheet' });
  await dialog.getByLabel('Size of the music (%)').fill('180');
  await dialog.getByLabel('Bar numbers').selectOption({ label: 'Over the first bar of each line' });
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect.poll(() => lines(page)).toBeGreaterThan(before);

  // One step to undo.
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => lines(page)).toBe(before);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect.poll(() => lines(page)).toBeGreaterThan(before);

  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Stylesheet…' }).click();
  await expect(dialog.getByLabel('Size of the music (%)')).toHaveValue('180');
  await dialog.getByRole('button', { name: 'As it comes' }).click();
  await expect(dialog.getByLabel('Size of the music (%)')).toHaveValue('100');
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect.poll(() => lines(page)).toBe(before);
});
