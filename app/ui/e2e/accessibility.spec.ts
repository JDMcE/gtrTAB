import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, twoTracks } from './app';

// What a screen reader and a keyboard are given. This is the part of a screen reader
// check that a machine can do: that every control has a name and a role that fits, that
// the page has its landmarks, and that the score, which is a picture, is also put in words.

async function violations(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

test('the app, its dialogs and its menus pass the automated checks', async ({ page }) => {
  // Five scans of the whole page take a while when the other tests are running too.
  test.slow();
  await twoTracks(page);
  expect(await violations(page)).toEqual([]);

  await page.getByRole('button', { name: 'Instrument, tuning, capo and mix…' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await keys(page, 'Escape');

  await keys(page, '?');
  await expect(page.getByRole('dialog')).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await keys(page, 'Escape');

  await page.getByRole('menuitem', { name: 'Beat' }).click();
  expect(await violations(page)).toEqual([]);
  await keys(page, 'Escape');
  // A chord goes on a beat, so on to one first: the guitar's.
  await keys(page, 'Control+ArrowUp');
  await page.getByRole('button', { name: 'Chord name and diagram…' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  expect(await violations(page)).toEqual([]);
});

test('the score is put in words as the cursor moves', async ({ page }) => {
  await twoTracks(page);
  const words = position(page);
  // It is a live region: what it says is read out when it changes, without taking the focus.
  await expect(words).toHaveAttribute('aria-live', 'polite');
  // The new track is where the cursor was, in the second bar, with nothing in it yet.
  await expect(words).toHaveText('Bar 2, empty, string 6, no note.');
  await fret(page, 3);
  await keys(page, 't');
  await expect(words).toHaveText('Bar 2, beat 1 of 1, eighth note, string 6, fret 3, G 2, tapped.');
  await keys(page, 'Control+ArrowUp');
  await expect(words).toHaveText('Bar 2, beat 1 of 2, eighth note, string 6, fret 3, G 2.');
  await keys(page, 'Control+Home');
  await expect(words).toHaveText('Bar 1, beat 1 of 8, eighth note, string 6, fret 0, E 2.');
  await keys(page, 'ArrowUp');
  await expect(words).toContainText('string 5, no note.');
});

test('every part of the window is reached from the keyboard', async ({ page }) => {
  await twoTracks(page);
  const regions = new Set<string>();
  for (let i = 0; i < 12; i++) {
    await keys(page, 'Tab');
    regions.add(
      await page.evaluate(() => {
        const at = document.activeElement!;
        const group = at.closest('[role=toolbar], [role=menubar], [role=application], .palette, .tracks, .inspector');
        return group?.getAttribute('aria-label') ?? group?.className.split(' ')[0] ?? at.tagName;
      }),
    );
  }
  for (const region of ['Score', 'File and edit', 'Playback and view', 'Tracks']) {
    expect([...regions]).toContain(region);
  }
  // Each is one stop, so a dozen presses of Tab go all the way round and more.
  expect(regions.size).toBeLessThanOrEqual(8);

  // In the track pane the arrows walk a row, and a slider is set without leaving the keys.
  await page.locator('.tracks .name').first().focus();
  await keys(page, 'ArrowRight', 'ArrowRight', 'ArrowRight');
  const volume = page.getByRole('slider', { name: 'Volume of Guitar' });
  await expect(volume).toBeFocused();
  await keys(page, 'ArrowDown', 'ArrowDown');
  await expect(volume).toHaveValue('73');
  await expect(volume).toBeFocused();
  // And Alt with an arrow moves the track, taking the focus with it.
  await page.locator('.tracks .name').first().focus();
  await keys(page, 'Alt+ArrowDown');
  await expect(page.locator('.tracks .name')).toHaveText(['Steel guitar', 'Guitar']);
  await expect(page.locator('.tracks .name').nth(1)).toBeFocused();
});
