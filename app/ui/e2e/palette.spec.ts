import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// The command palette, which the plan's tenth phase asks for: every command in the menus,
// found by typing a few letters of its name.

const palette = (page: Page) => page.getByRole('dialog', { name: 'Command palette' });
const field = (page: Page) => palette(page).getByRole('combobox', { name: 'Command' });
const options = (page: Page) => palette(page).getByRole('option');

async function violations(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).include('dialog').analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

test('a command is found by a few letters and run from the keyboard', async ({ page }) => {
  await start(page);
  await fret(page, 5);

  await keys(page, 'Control+k');
  await expect(field(page)).toBeFocused();
  expect(await violations(page)).toEqual([]);

  // A few letters of each word, and the menus it is under beside it.
  await page.keyboard.type('pal mu');
  await expect(options(page).first()).toContainText('Palm mute');
  await expect(options(page).first()).toContainText('Note');
  await expect(options(page).first()).toHaveAttribute('aria-selected', 'true');
  await keys(page, 'Enter');
  await expect(palette(page)).toBeHidden();
  await expect(position(page)).toContainText('palm mute.');

  // Back on the score, the keys work as before.
  await keys(page, 'ArrowRight');
  await fret(page, 7);
  await expect(position(page)).toContainText('beat 2');

  // What was run comes first next time, and shows that it is on.
  await keys(page, 'Control+Shift+P');
  await keys(page, 'ArrowLeft');
  await expect(options(page).first()).toContainText('Palm mute');
  await keys(page, 'Escape');
  await expect(palette(page)).toBeHidden();
});

test('the arrows move through what is found, and what cannot be used is not run', async ({ page }) => {
  await start(page);
  await page.getByRole('menuitem', { name: 'Help', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Command palette…' }).click();
  await expect(field(page)).toBeFocused();

  await page.keyboard.type('theme');
  await expect(options(page)).toHaveText([/System/, /Light/, /Dark/]);
  await keys(page, 'ArrowDown', 'ArrowDown');
  await expect(options(page).nth(2)).toHaveAttribute('aria-selected', 'true');
  await expect(field(page)).toHaveAttribute('aria-activedescendant', await options(page).nth(2).getAttribute('id') ?? '');
  await keys(page, 'Enter');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  // Nothing to paste yet: it is shown, but last and greyed, and Enter leaves the palette open.
  await keys(page, 'Control+k');
  await page.keyboard.type('paste');
  await expect(options(page).last()).toHaveAttribute('aria-disabled', 'true');
  await keys(page, 'Enter');
  await expect(palette(page)).toBeVisible();
  await options(page).last().click({ force: true });
  await expect(palette(page)).toBeVisible();

  await field(page).fill('no such command');
  await expect(palette(page)).toContainText('No command matches');
  await keys(page, 'Escape');
});
