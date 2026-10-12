import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// What came late from Guitar Pro can be written here too: the shortest notes, fades, a
// second segno and coda, several jumps on a bar, the dotted and Scottish feels, and a
// tempo that changes by degrees.

/** An entry of an open menu, whether or not it is one that can be ticked. */
const entry = (page: Page, name: string) =>
  page
    .getByRole('menuitem', { name, exact: true })
    .or(page.getByRole('menuitemcheckbox', { name, exact: true }));

/** Picks an entry of a menu, by way of a submenu if one is named. */
async function choose(page: Page, menu: string, ...path: string[]) {
  await page.getByRole('menuitem', { name: menu, exact: true }).click();
  for (const name of path) await entry(page, name).click();
}

/** Whether an entry of a submenu is ticked. */
async function ticked(page: Page, menu: string, submenu: string, entry: string) {
  await page.getByRole('menuitem', { name: menu, exact: true }).click();
  await page.getByRole('menuitem', { name: submenu, exact: true }).click();
  const state = await page.getByRole('menuitemcheckbox', { name: entry, exact: true }).getAttribute('aria-checked');
  await keys(page, 'Escape', 'Escape', 'Escape');
  return state === 'true';
}

test('the shortest notes and fades are written', async ({ page }) => {
  await start(page);
  // Six steps shorter than a quarter note is a 256th.
  await keys(page, '+', '+', '+', '+', '+', '+');
  await fret(page, 5);
  await expect(position(page)).toContainText('two hundred and fifty-sixth note, string 6, fret 5');
  await keys(page, '+');
  await expect(position(page)).toContainText('two hundred and fifty-sixth note');
  await keys(page, '-');
  await expect(position(page)).toContainText('hundred and twenty-eighth note');

  await choose(page, 'Beat', 'Fade', 'Volume swell');
  await expect(position(page)).toContainText('note, swelling, string 6');
  await choose(page, 'Beat', 'Fade', 'Fade in');
  await expect(position(page)).toContainText('note, fading in, string 6');
  await choose(page, 'Beat', 'Fade', 'Fade in');
  await expect(position(page)).not.toContainText('fading');
});

test('a bar takes several signs and jumps, and the new feels', async ({ page }) => {
  await start(page);
  await fret(page, 3);
  await choose(page, 'Score', 'Segno, coda and fine', 'Second segno');
  await choose(page, 'Score', 'Segno, coda and fine', 'Second coda');
  await choose(page, 'Score', 'Jumps at the end of the bar', 'D.S.S. al Double Coda');
  await choose(page, 'Score', 'Jumps at the end of the bar', 'To Double Coda');
  for (const [submenu, entry] of [
    ['Segno, coda and fine', 'Second segno'],
    ['Segno, coda and fine', 'Second coda'],
    ['Jumps at the end of the bar', 'D.S.S. al Double Coda'],
    ['Jumps at the end of the bar', 'To Double Coda'],
  ]) {
    expect(await ticked(page, 'Score', submenu, entry), entry).toBe(true);
  }
  expect(await ticked(page, 'Score', 'Segno, coda and fine', 'Segno')).toBe(false);
  // Chosen again, one comes off and the other stays.
  await choose(page, 'Score', 'Segno, coda and fine', 'Second segno');
  expect(await ticked(page, 'Score', 'Segno, coda and fine', 'Second segno')).toBe(false);
  expect(await ticked(page, 'Score', 'Segno, coda and fine', 'Second coda')).toBe(true);

  await choose(page, 'Score', 'Feel', 'Scottish eighths (short, long)');
  expect(await ticked(page, 'Score', 'Feel', 'Scottish eighths (short, long)')).toBe(true);
  await expect(page.locator('footer.failed')).toHaveCount(0);
});

test('a tempo can change by degrees', async ({ page }) => {
  await start(page);
  await fret(page, 3);
  const gradual = entry(page, 'Change tempo by degrees, up to the next change');
  // There has to be a change of tempo on the beat for it to be gradual.
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await expect(gradual).toBeDisabled();
  await entry(page, 'Tempo change…').click();
  await page.getByLabel('Tempo (bpm)').fill('90');
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await expect(gradual).toBeEnabled();
  await gradual.click();
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await expect(gradual).toHaveAttribute('aria-checked', 'true');
  await keys(page, 'Escape');
  // And it plays: the engine takes the score as it is.
  await keys(page, 'Space');
  await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible();
  await keys(page, 'Space');
  await expect(page.locator('footer.failed')).toHaveCount(0);
});
