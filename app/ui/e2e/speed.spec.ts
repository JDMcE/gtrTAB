import { expect, test, type Page } from '@playwright/test';
import { fret, keys, start } from './app';

// Playing slower or faster than the tempo written, which the plan's eighth phase asks for.

const chip = (page: Page) => page.getByRole('button', { name: /^Playback speed:/ });

async function pickSpeed(page: Page, item: string): Promise<void> {
  await page.getByRole('menuitem', { name: 'Score', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Playback speed', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: item, exact: true }).click();
}

test('the speed is set from the keys, the menu and the transport bar', async ({ page }) => {
  await start(page);
  await expect(chip(page)).toHaveText('100%');

  await keys(page, '[', '[');
  await expect(chip(page)).toHaveText('90%');
  await expect(page.getByRole('status')).toHaveText('Playing at 90% of the tempo');
  await keys(page, ']');
  await expect(chip(page)).toHaveText('95%');

  await pickSpeed(page, '50%');
  await expect(chip(page)).toHaveText('50%');

  // The chip asks for any speed, and keeps what it is given within what can be played.
  await chip(page).click();
  await page.getByLabel('Speed (%)').fill('62');
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(chip(page)).toHaveText('62%');

  await pickSpeed(page, '25%');
  await keys(page, '[');
  await expect(chip(page)).toHaveText('25%');
  await pickSpeed(page, '200%');
  await keys(page, ']');
  await expect(chip(page)).toHaveText('200%');
  await expect(page.locator('footer.failed')).toHaveCount(0);
});

test('a change of speed is heard in what is already playing', async ({ page }) => {
  await start(page);
  // Two bars of quarter notes: four seconds as written, sixteen at a quarter of the speed.
  for (let note = 0; note < 8; note++) {
    await fret(page, 5);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
  await pickSpeed(page, '25%');
  await keys(page, 'Space');
  const stopButton = page.getByRole('button', { name: 'Stop' });
  await expect(stopButton).toBeVisible();

  // Still in the first bar; at twice the speed the rest is over in two seconds.
  await page.waitForTimeout(1000);
  await pickSpeed(page, '200%');
  await expect(stopButton).toBeHidden({ timeout: 5000 });
  await expect(page.locator('footer.failed')).toHaveCount(0);
});
