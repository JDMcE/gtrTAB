import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// Preferences, themes and the start of translation, which the plan's tenth phase asks for.

const dialog = (page: Page) => page.getByRole('dialog', { name: /Preferences|Pŕéféŕéñçéš/ });
const html = (page: Page) => page.locator('html');

async function violations(page: Page, include?: string): Promise<string[]> {
  const builder = new AxeBuilder({ page });
  if (include) builder.include(include);
  const { violations } = await builder.analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

test('the preferences set the theme, contrast and paper, and are kept', async ({ page }) => {
  await start(page);
  await fret(page, 5);
  await keys(page, 'Control+,');
  await expect(dialog(page)).toBeVisible();
  expect(await violations(page, 'dialog')).toEqual([]);

  await dialog(page).getByRole('combobox', { name: 'Theme' }).selectOption({ label: 'Dark' });
  await expect(html(page)).toHaveAttribute('data-theme', 'dark');
  await dialog(page).getByRole('combobox', { name: 'Contrast' }).selectOption({ label: 'High' });
  await expect(html(page)).toHaveAttribute('data-contrast', 'high');
  await dialog(page).getByRole('combobox', { name: 'The score' }).selectOption({ label: 'On white paper, as it is printed' });
  await expect(html(page)).toHaveAttribute('data-paper', 'white');

  // The score is drawn on white in a dark theme.
  const paper = await page.locator('div[data-paper]').evaluate((el) => {
    const probe = el.appendChild(document.createElement('span'));
    probe.style.color = 'var(--paper)';
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });
  expect(paper).toBe('rgb(255, 255, 255)');

  await dialog(page).getByRole('button', { name: 'Done' }).click();
  await page.reload();
  await expect(position(page)).toContainText('fret 5');
  await expect(html(page)).toHaveAttribute('data-contrast', 'high');
  await expect(html(page)).toHaveAttribute('data-paper', 'white');

  // Back as they came, keeping the keys given to commands.
  await page.getByRole('menuitem', { name: 'Edit', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Preferences…' }).click();
  await dialog(page).getByRole('button', { name: 'Reset preferences' }).click();
  await expect(html(page)).not.toHaveAttribute('data-contrast', 'high');
  await expect(html(page)).not.toHaveAttribute('data-theme');
  await expect(html(page)).not.toHaveAttribute('data-paper');
});

test('the system asking for more contrast gets high contrast, unless the preferences say otherwise', async ({ page }) => {
  await page.emulateMedia({ contrast: 'more' });
  await start(page);
  await expect(html(page)).toHaveAttribute('data-contrast', 'high');
  await keys(page, 'Control+,');
  await dialog(page).getByRole('combobox', { name: 'Contrast' }).selectOption({ label: 'Normal' });
  await expect(html(page)).not.toHaveAttribute('data-contrast');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`high contrast in the ${scheme} theme passes the automated checks`, async ({ page }) => {
    test.slow();
    await page.emulateMedia({ colorScheme: scheme, contrast: 'more' });
    await start(page);
    await fret(page, 3);
    expect(await violations(page)).toEqual([]);
  });
}

test('in the pseudo-language every text translated is changed, and the score stays open', async ({ page }) => {
  await start(page);
  await fret(page, 7);
  await keys(page, 'Control+,');
  await dialog(page).getByRole('combobox', { name: 'Language' }).selectOption({ label: 'Pseudo-language, to find texts not translated' });
  await expect(dialog(page)).toContainText('changes language when gtrTab starts again');
  await dialog(page).getByRole('button', { name: 'Start again now' }).click();

  // The menus are in the pseudo-language, and the work is still there, not saved.
  await expect(page.getByRole('menuitem', { name: /^\[Fíļé·+\]$/ })).toBeVisible();
  await expect(position(page)).toContainText('fret 7');
  await expect(page.locator('footer')).toContainText('[Ŕéçövéŕéð üñšávéð çĥáñĝéš ţö untitled.gtrtab');
  await page.getByRole('menuitem', { name: /^\[Éðíţ·+\]$/ }).click();
  await expect(page.getByRole('menuitem', { name: /^\[Pŕéféŕéñçéš…/ })).toBeVisible();
  await keys(page, 'Escape', 'Escape');

  // Back to English for the next start.
  await keys(page, 'Control+,');
  await dialog(page).getByRole('combobox').nth(3).selectOption({ value: 'en' });
  await dialog(page).getByRole('button').filter({ hasText: /Šţáŕţ/ }).click();
  await expect(page.getByRole('menuitem', { name: 'File', exact: true })).toBeVisible();
  await expect(html(page)).toHaveAttribute('lang', 'en');
});
