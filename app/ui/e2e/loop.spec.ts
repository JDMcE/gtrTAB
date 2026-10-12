import { expect, test, type Page } from '@playwright/test';
import { fret, keys, start } from './app';

// Looping a passage, and the speed trainer that plays it faster each time round, which the
// plan's eighth phase asks for.

const chip = (page: Page) => page.getByRole('button', { name: /^Playback speed:/ });
const stopButton = (page: Page) => page.getByRole('button', { name: 'Stop' });

/** A bar of quarter notes: two seconds as written, a second at twice the speed. */
async function oneBar(page: Page): Promise<void> {
  await start(page);
  for (let note = 0; note < 4; note++) {
    await fret(page, 5);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
}

async function setSpeed(page: Page, percent: number): Promise<void> {
  await chip(page).click();
  await page.getByLabel('Speed (%)').fill(String(percent));
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(chip(page)).toHaveText(`${percent}%`);
}

test('a looped passage plays until stopped, or to its end once the loop is off', async ({ page }) => {
  await oneBar(page);
  await setSpeed(page, 200);
  const loop = page.getByRole('button', { name: 'Loop', exact: true });
  await loop.click();
  await expect(loop).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('status')).toHaveText('Looping');

  await keys(page, 'Space');
  await expect(stopButton(page)).toBeVisible();
  // Three times through the bar, and still going.
  await page.waitForTimeout(3000);
  await expect(stopButton(page)).toBeVisible();

  // Out of the loop, it finishes the time round it is on and stops.
  await loop.click();
  await expect(loop).toHaveAttribute('aria-pressed', 'false');
  await expect(stopButton(page)).toBeHidden({ timeout: 3000 });
  await expect(page.locator('footer.failed')).toHaveCount(0);
});

test('the speed trainer plays the loop faster each time round, up to the speed set', async ({ page }) => {
  await oneBar(page);
  await setSpeed(page, 150);
  const trainer = page.getByRole('button', { name: 'Speed trainer…', exact: true });
  await trainer.click();
  await page.getByLabel('Faster by (%)').fill('25');
  await page.getByLabel('After this many times round').fill('1');
  await page.getByLabel('Up to (%)').fill('200');
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(trainer).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('button', { name: 'Loop', exact: true })).toHaveAttribute('aria-pressed', 'true');

  await keys(page, 'Space');
  await expect(stopButton(page)).toBeVisible();
  await expect(chip(page)).toHaveText('175%', { timeout: 3000 });
  await expect(page.getByRole('status')).toHaveText('Time 2 round, at 175% of the tempo');
  await expect(chip(page)).toHaveText('200%', { timeout: 3000 });
  // At the speed it was going up to, it goes on round at that speed.
  await page.waitForTimeout(2500);
  await expect(chip(page)).toHaveText('200%');
  await expect(stopButton(page)).toBeVisible();
  await keys(page, 'Space');
  await expect(stopButton(page)).toBeHidden();

  // Switched off from its dialog, and the loop with it stays as it was.
  await trainer.click();
  await page.getByRole('button', { name: 'Turn off', exact: true }).click();
  await expect(trainer).toHaveAttribute('aria-pressed', 'false');
  await expect(page.getByRole('status')).toHaveText('Speed trainer off');
  await expect(page.locator('footer.failed')).toHaveCount(0);
});
