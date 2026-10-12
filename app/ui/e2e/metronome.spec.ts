import { expect, test, type Page } from '@playwright/test';
import { fret, keys, start } from './app';

// The visual metronome, which the plan's eighth phase asks for: a light for each pulse of
// the bar, lit in time with the music and the count-in.

const lights = (page: Page) => page.getByRole('img', { name: 'Visual metronome' });

test('the lights count the bar as it plays, the count-in first', async ({ page }) => {
  await start(page);
  for (let note = 0; note < 4; note++) {
    await fret(page, 3);
    await keys(page, 'ArrowRight');
  }
  await keys(page, 'Control+Home');
  await expect(lights(page)).toHaveCount(0);
  const button = page.getByRole('button', { name: 'Visual metronome', exact: true });
  await button.click();
  await expect(button).toHaveAttribute('aria-pressed', 'true');
  // Four lights for a bar of 4/4, none of them lit before playing.
  await expect(lights(page).locator('.light')).toHaveCount(4);
  await expect(lights(page)).toHaveAttribute('data-pulse', '');

  await page.getByRole('button', { name: 'Count-in bar', exact: true }).click();
  await keys(page, 'Space');
  // A count-in bar and a bar of music, two seconds each.
  await expect(lights(page)).toHaveAttribute('data-count-in', 'true');
  await expect(lights(page)).toHaveAttribute('data-pulse', '3');
  await expect(lights(page)).toHaveAttribute('data-count-in', 'false');
  await expect(lights(page)).toHaveAttribute('data-pulse', '1');
  await expect(lights(page)).toHaveAttribute('data-pulse', '2');
  await expect(lights(page)).toHaveAttribute('data-pulse', '4');
  await expect(page.getByRole('button', { name: 'Stop' })).toBeHidden({ timeout: 3000 });
  await expect(lights(page)).toHaveAttribute('data-pulse', '');

  // It stays where it was put the next time the app is opened.
  await page.reload();
  await expect(lights(page)).toBeVisible();
});
