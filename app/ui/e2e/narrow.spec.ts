import { expect, test, type Page } from '@playwright/test';
import { place, position, twoTracks } from './app';

// A narrow window: 640 pixels is the least the desktop window goes to, and a browser can
// be narrower still. Nothing may be pushed off the side, and the track pane and every
// notation view have to go on working.

/** Controls that are shown but lie outside the window, sideways. */
const offTheSide = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('button, input, select, [role^=menuitem]')]
      .filter((control) => control.offsetParent !== null)
      .filter((control) => {
        const box = control.getBoundingClientRect();
        // What scrolls sideways inside a pane of its own, like the bar grid, is not lost.
        const scrolls = control.closest('.scroller');
        return !scrolls && (box.left < -1 || box.right > window.innerWidth + 1);
      })
      .map((control) => control.getAttribute('aria-label') ?? control.textContent?.trim()),
  );

/** How much of the canvas is not paper: something has been drawn on it. */
const inked = (page: Page) =>
  page.evaluate(() => {
    const canvas = document.querySelector('canvas')!;
    const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data;
    let dark = 0;
    for (let i = 0; i < pixels.length; i += 4) if (pixels[i] + pixels[i + 1] + pixels[i + 2] < 300) dark++;
    return dark;
  });

for (const width of [640, 420]) {
  test(`everything stays in reach in a window ${width} pixels wide`, async ({ page }) => {
    await page.setViewportSize({ width, height: 700 });
    await twoTracks(page);

    const spill = await page.evaluate(() => {
      const root = document.documentElement;
      return { wide: root.scrollWidth - root.clientWidth, tall: root.scrollHeight - root.clientHeight };
    });
    expect(spill).toEqual({ wide: 0, tall: 0 });
    expect(await offTheSide(page)).toEqual([]);

    // The track pane: both tracks, each with its switches, and a click changes track.
    const names = page.locator('.tracks .name');
    await expect(names).toHaveText(['Guitar', 'Steel guitar']);
    await expect(page.getByRole('button', { name: 'Mute Guitar' })).toBeInViewport();
    await expect(page.getByRole('button', { name: 'Solo Steel guitar' })).toBeInViewport();
    await names.first().click();
    await page.keyboard.press('Control+Home');
    await expect(place(page)).toContainText('Guitar · Voice 1 · Bar 1 of 2');
    await expect(position(page)).toContainText('beat 1 of 8, eighth note, string 6, fret 0, E 2.');

    // Each notation view draws, and still nothing is off the side.
    const drawn: Record<string, number> = {};
    for (const view of ['Tab only', 'Notation only', 'Rhythm slashes', 'Notation and tab']) {
      await page.getByRole('menuitem', { name: 'View' }).click();
      await page.getByRole('menuitemcheckbox', { name: view }).click();
      await expect.poll(() => inked(page)).toBeGreaterThan(400);
      drawn[view] = await inked(page);
      expect(await offTheSide(page)).toEqual([]);
    }
    expect(drawn['Notation and tab']).toBeGreaterThan(drawn['Tab only']);

    // Every track at once.
    await page.getByRole('menuitem', { name: 'View' }).click();
    await page.getByRole('menuitemcheckbox', { name: 'Show every track' }).click();
    await expect.poll(() => inked(page)).not.toBe(drawn['Notation and tab']);
    expect(await inked(page)).toBeGreaterThan(400);
    expect(await offTheSide(page)).toEqual([]);
  });
}
