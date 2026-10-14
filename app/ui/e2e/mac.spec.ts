import { expect, test } from '@playwright/test';
import { keys, start } from './app';

// On a Mac the keys are shown with its own symbols, and Cmd does what Ctrl does elsewhere.

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'platform', { get: () => 'MacIntel' });
    Object.defineProperty(navigator, 'userAgentData', { get: () => ({ platform: 'macOS' }) });
  });
});

test('the menus, the shortcuts and a screen reader have the keys as a Mac has them', async ({ page }) => {
  await start(page);
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  const saveAs = page.getByRole('menuitem', { name: 'Save as…' });
  await expect(saveAs.locator('kbd')).toHaveText('⇧⌘S');
  await expect(saveAs).toHaveAttribute('aria-keyshortcuts', 'Meta+Shift+S');
  await keys(page, 'Escape', 'Escape');

  await keys(page, 'Meta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.type('redo');
  await expect(palette.getByRole('option').first().locator('kbd')).toHaveText('⌘Y');
  await keys(page, 'Escape');

  await keys(page, '?');
  const list = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
  await expect(list.getByRole('button', { name: 'Take ⇧⌘Z from Redo' })).toBeVisible();
  await expect(list).toContainText('⌥1');
});
