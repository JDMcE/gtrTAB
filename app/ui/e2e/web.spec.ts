import { expect, test, type Page } from '@playwright/test';
import { fret, keys, position, start } from './app';

// The web app: it opens and works with no connection once it has been opened with one, and
// where the browser lets a page keep hold of files on disk, a score is saved back to where
// it came from and opened again from the recent files, as on desktop.

test('the app opens and exports with no connection, once it has been opened with one', async ({ page, context }) => {
  await start(page);
  // The service worker has kept the app's files once it controls the page.
  await expect.poll(() => page.evaluate(() => !!navigator.serviceWorker.controller), { timeout: 15_000 }).toBe(true);
  await context.setOffline(true);
  await page.reload();
  await expect(position(page)).toHaveText(/^Bar 1, empty/);
  await fret(page, 7);
  // The fonts a PDF is drawn with are kept too.
  const download = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
  await page.getByRole('menuitem', { name: 'PDF…', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('untitled.pdf');
  await context.setOffline(false);
});

/**
 * Answers the browser's pickers from its private file system, whose files are real ones
 * that a page can keep hold of: the file picked to open is `window.pick`, a file saved is
 * named as suggested, and the folder picked is `converted`.
 */
async function pickers(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const root = () => navigator.storage.getDirectory();
    const w = window as unknown as Record<string, unknown>;
    w.picked = [];
    w.showOpenFilePicker = async () => {
      (w.picked as string[]).push('open');
      return [await (await root()).getFileHandle(w.pick as string)];
    };
    w.showSaveFilePicker = async ({ suggestedName }: { suggestedName: string }) => {
      (w.picked as string[]).push(`save ${suggestedName}`);
      return (await root()).getFileHandle(suggestedName, { create: true });
    };
    w.showDirectoryPicker = async () => {
      (w.picked as string[]).push('folder');
      return (await root()).getDirectoryHandle('converted', { create: true });
    };
  });
}

/** The names of the files in the private file system, and those in its folder `converted`. */
const disk = (page: Page) =>
  page.evaluate(async () => {
    const names = async (dir: FileSystemDirectoryHandle) => {
      const found: string[] = [];
      for await (const key of (dir as unknown as { keys(): AsyncIterable<string> }).keys()) found.push(key);
      return found.sort();
    };
    const root = await navigator.storage.getDirectory();
    return { root: await names(root), converted: await names(await root.getDirectoryHandle('converted', { create: true })) };
  });

/** What the pickers have been asked for since last asked. */
const asked = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { picked: string[] };
    return w.picked.splice(0);
  });

test('a score is saved back to the file it came from, and opened again from the recent files', async ({ page }) => {
  await pickers(page);
  await start(page, { pickers: true });
  await fret(page, 5);

  // A new score is saved where the user says, and after that where it was.
  await keys(page, 'Control+s');
  await expect(page.getByRole('status')).toHaveText('Saved untitled.gtrtab');
  expect(await asked(page)).toEqual(['save untitled.gtrtab']);
  await keys(page, 'ArrowRight');
  await fret(page, 7);
  await keys(page, 'Control+s');
  await expect(page.getByRole('status')).toHaveText('Saved untitled.gtrtab');
  expect(await asked(page)).toEqual([]);
  expect((await disk(page)).root.filter((name) => name.endsWith('.gtrtab'))).toEqual(['untitled.gtrtab']);

  // Opened again from the recent files, after the page has been closed, with both notes.
  await page.reload();
  await expect(position(page)).toHaveText(/^Bar 1, empty/);
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Open recent', exact: true }).click();
  await page.getByRole('menuitem', { name: 'untitled.gtrtab', exact: true }).click();
  await expect(page.locator('.file')).toHaveText('untitled.gtrtab');
  await keys(page, 'ArrowRight');
  await expect(position(page)).toContainText('fret 7,');

  // Opened from the picker, it is the same file, not another in the recent list.
  await page.evaluate(() => ((window as unknown as { pick: string }).pick = 'untitled.gtrtab'));
  await keys(page, 'Control+o');
  await expect(position(page)).toContainText('fret 5,');
  expect(await asked(page)).toEqual(['open']);
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Open recent', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: 'untitled.gtrtab' })).toHaveCount(1);
  await keys(page, 'Escape', 'Escape');
});

test('exports are saved where the user says, and a batch is written into a folder', async ({ page }) => {
  await pickers(page);
  await start(page, { pickers: true });
  await fret(page, 3);
  await keys(page, 'Control+s');
  await expect(page.getByRole('status')).toHaveText('Saved untitled.gtrtab');

  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
  await page.getByRole('menuitem', { name: 'MIDI…', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Exported untitled.mid');
  expect(await asked(page)).toEqual(['save untitled.gtrtab', 'save untitled.mid']);

  await page.evaluate(() => ((window as unknown as { pick: string }).pick = 'untitled.gtrtab'));
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Batch convert…', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Batch convert' });
  await expect(dialog).toContainText('into a folder you choose');
  await dialog.getByRole('button', { name: 'Add scores…' }).click();
  await dialog.getByLabel('Convert to').selectOption({ label: 'Text tab' });
  await dialog.getByRole('button', { name: 'Convert', exact: true }).click();
  await expect(dialog.getByRole('status')).toContainText('1 score converted.');
  // Converted twice, the second is numbered rather than written over the first.
  await dialog.getByRole('button', { name: 'Convert', exact: true }).click();
  await expect(dialog.getByRole('list', { name: 'Converted' })).toContainText('untitled.gtrtab → untitled (2).txt');
  expect((await disk(page)).converted).toEqual(['untitled (2).txt', 'untitled.txt']);
  expect((await asked(page)).filter((a) => a === 'folder')).toHaveLength(2);
});
