import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// Keyboard shortcuts the user chooses, which the plan's tenth phase asks for: any command
// in the menus can be given a key, or have its keys taken away, and the choice is kept.

const list = (page: Page) => page.getByRole('dialog', { name: 'Keyboard shortcuts' });
const said = (page: Page) => list(page).getByRole('status');

async function violations(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).include('dialog').analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

test('a command is given a key, taken from the one that had it, and the choice is kept', async ({ page }) => {
  await start(page);
  await fret(page, 5);

  await keys(page, '?');
  await expect(list(page)).toBeVisible();
  expect(await violations(page)).toEqual([]);

  // Staccato has no key of its own: give it P, which palm mute had.
  await list(page).getByRole('searchbox', { name: 'Find a command' }).fill('staccato');
  await list(page).getByRole('button', { name: 'Give Staccato a key' }).click();
  await expect(said(page)).toHaveText('Press the key for Staccato, or Esc.');
  // The arrows move the cursor, and are not given.
  await keys(page, 'ArrowLeft');
  await expect(said(page)).toContainText('cannot be given to a command');
  await keys(page, 'p');
  await expect(said(page)).toHaveText('P now runs Staccato, and no longer Palm mute.');
  await expect(list(page).getByRole('button', { name: 'Take P from Staccato' })).toBeVisible();
  await list(page).getByRole('button', { name: 'Done' }).click();
  await expect(list(page)).toBeHidden();

  await keys(page, 'p');
  await expect(position(page)).toContainText('staccato');
  await expect(position(page)).not.toContainText('palm mute');

  // The menus show the key where it is now.
  await page.getByRole('menuitem', { name: 'Note', exact: true }).click();
  await expect(page.getByRole('menuitemcheckbox', { name: 'Staccato' })).toHaveAttribute('aria-keyshortcuts', 'P');
  await expect(page.getByRole('menuitemcheckbox', { name: 'Palm mute' })).not.toHaveAttribute('aria-keyshortcuts');
  await keys(page, 'Escape', 'Escape');

  // Kept for next time. The score comes back too, as work not saved; a new note goes after it.
  await page.reload();
  await expect(position(page)).toContainText('fret 5');
  await keys(page, 'ArrowRight');
  await fret(page, 3);
  await keys(page, 'p');
  await expect(position(page)).toContainText('fret 3, G 2, staccato');

  // Back as they came.
  await keys(page, 'F1');
  await list(page).getByRole('button', { name: 'Reset every key' }).click();
  await expect(said(page)).toHaveText('Every command has the keys it came with.');
  await keys(page, 'Escape');
  await expect(list(page)).toBeHidden();
  await keys(page, 'p');
  await expect(position(page)).toContainText('fret 3, G 2, palm mute, staccato');
});

test('a key is taken away, given back, and Esc while waiting gives none', async ({ page }) => {
  await start(page);
  await fret(page, 5);
  await keys(page, '?');

  await list(page).getByRole('button', { name: 'Take R from Rest' }).click();
  await expect(said(page)).toHaveText('R no longer runs Rest.');

  // Esc while a command waits for its key gives none, and leaves the list open.
  await list(page).getByRole('button', { name: 'Give Rest a key' }).click();
  await keys(page, 'Escape');
  await expect(said(page)).toHaveText('No key given.');
  await expect(list(page)).toBeVisible();

  await list(page).getByRole('button', { name: 'Done' }).click();
  await keys(page, 'r');
  await expect(position(page)).toContainText('fret 5');

  // Every command, with or without a key, can be found; this one is given its own back.
  await keys(page, '?');
  await list(page).getByRole('button', { name: 'Give Rest its own keys back' }).click();
  await expect(said(page)).toHaveText('Rest has R again.');
  await list(page).getByRole('button', { name: 'Done' }).click();
  await keys(page, 'r');
  await expect(position(page)).toContainText('rest');
});
