import { expect, type Page } from '@playwright/test';

/**
 * Opens the app and waits for an empty score with the cursor on it. Unless `pickers` is
 * set, the browser's pickers of files on disk are hidden, as Playwright cannot answer
 * them, so that files are picked and saved through uploads and downloads.
 */
export async function start(page: Page, { pickers = false } = {}): Promise<void> {
  if (!pickers) {
    await page.addInitScript(() => {
      for (const name of ['showOpenFilePicker', 'showSaveFilePicker', 'showDirectoryPicker']) {
        Object.defineProperty(window, name, { value: undefined, configurable: true });
      }
    });
  }
  await page.goto('/');
  await expect(position(page)).toHaveText(/^Bar 1, empty/);
}

/** What the app says is under the cursor: the words a screen reader is given. */
export const position = (page: Page) => page.locator('[data-position]');

/** The place in the score shown in the status bar: track, voice, bar and beat. */
export const place = (page: Page) => page.locator('footer span').last();

/** Types keys one after the other, as at a keyboard. */
export async function keys(page: Page, ...sequence: string[]): Promise<void> {
  for (const key of sequence) await page.keyboard.press(key);
}

/** Enters a fret and waits for it to be on the score: two digits typed quickly are one number. */
export async function fret(page: Page, number: number): Promise<void> {
  await page.keyboard.type(String(number));
  await expect(position(page)).toContainText(`fret ${number},`);
  // Let the wait for a second digit run out before the next note is typed.
  await page.waitForTimeout(450);
}

/** Writes a bar and a quarter of eighth notes and adds a second track, which becomes the one edited. */
export async function twoTracks(page: Page): Promise<void> {
  await start(page);
  await keys(page, '+');
  for (const number of [0, 2, 3, 5, 7, 5, 3, 2, 3, 5]) {
    await fret(page, number);
    await keys(page, 'ArrowRight');
  }
  await page.getByRole('button', { name: 'Add a track…' }).click();
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(place(page)).toContainText('Steel guitar');
}
