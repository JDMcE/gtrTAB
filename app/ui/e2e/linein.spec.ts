import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { start } from './app';

// The line in, which the plan's ninth phase asks for: a guitar played through the pedals
// of the track and of the mix. The guitar is stood in for by a tone made in the page.

/** Puts a tone where the input would be, or with `refuse` an input that will not open. */
async function fakeInput(page: Page, refuse = false): Promise<void> {
  await page.addInitScript((refuse) => {
    const w = window as unknown as Record<string, unknown>;
    w.asked = [];
    navigator.mediaDevices.getUserMedia = async (constraints) => {
      (w.asked as unknown[]).push(constraints);
      if (refuse) throw new DOMException('Permission denied', 'NotAllowedError');
      const context = new AudioContext();
      const oscillator = context.createOscillator();
      oscillator.type = 'sawtooth';
      oscillator.frequency.value = 110;
      const gain = context.createGain();
      gain.gain.value = 0.2;
      const out = context.createMediaStreamDestination();
      oscillator.connect(gain).connect(out);
      oscillator.start();
      return out.stream;
    };
    navigator.mediaDevices.enumerateDevices = async () =>
      [
        { kind: 'audioinput', deviceId: 'default', label: 'Default', groupId: 'a' },
        { kind: 'audioinput', deviceId: 'line', label: 'Line in', groupId: 'b' },
      ] as MediaDeviceInfo[];
  }, refuse);
}

async function openLineIn(page: Page) {
  await page.getByRole('menuitem', { name: 'Tools', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Line in…', exact: true }).click();
  return page.getByRole('dialog', { name: 'Line in' });
}

const level = async (dialog: ReturnType<Page['getByRole']>, side: 'input' | 'output') =>
  Number(await dialog.locator('.meters').getAttribute(`data-${side}`));

test('a guitar on the line in is heard through the pedals, until it is stopped', async ({ page }) => {
  await fakeInput(page);
  await start(page);
  let dialog = await openLineIn(page);
  await expect(dialog.locator('[data-track-pedals]')).toHaveText('no pedals');
  const { violations } = await new AxeBuilder({ page }).include('dialog').analyze();
  expect(violations.map((v) => v.id)).toEqual([]);

  // The line in chosen, and the sound as it is: not cleaned up for speech.
  await dialog.getByLabel('Input').selectOption({ label: 'Line in' });
  await dialog.getByRole('button', { name: 'Start' }).click();
  await expect(dialog.locator('.meters')).toBeVisible();
  const asked = await page.evaluate(() => (window as unknown as { asked: { audio: Record<string, unknown> }[] }).asked);
  expect(asked[0].audio).toMatchObject({ deviceId: { exact: 'line' }, echoCancellation: false, autoGainControl: false });
  await expect.poll(() => level(dialog, 'input')).toBeGreaterThan(0.1);
  await expect.poll(() => level(dialog, 'output')).toBeGreaterThan(0.1);
  await expect(dialog.locator('[data-latency]')).toContainText(/Heard about \d+ ms|not known/);

  // Brought down by the gain, the line is quieter going in.
  const before = await level(dialog, 'input');
  await dialog.getByLabel('Gain (dB)').fill('-12');
  await dialog.getByLabel('Gain (dB)').press('Tab');
  await expect.poll(() => level(dialog, 'input')).toBeLessThan(before / 2);

  // It carries on with the dialog closed, and follows the pedals set meanwhile.
  await dialog.getByRole('button', { name: 'Show the pedalboard' }).click();
  await expect(dialog).toBeHidden();
  const stop = page.getByRole('button', { name: 'Line in: playing' });
  await expect(stop).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('region', { name: 'Pedalboard' }).getByRole('combobox', { name: 'Put on a preset' }).selectOption({ label: 'Crunch' });
  dialog = await openLineIn(page);
  await expect(dialog.locator('[data-track-pedals]')).toHaveText('Amp → Cabinet → Reverb');
  await expect.poll(() => level(dialog, 'output')).toBeGreaterThan(0.05);
  await dialog.getByRole('button', { name: 'Stop' }).click();
  await expect(dialog.getByRole('button', { name: 'Start' })).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(stop).toBeHidden();
  // The gain is kept for next time.
  dialog = await openLineIn(page);
  await expect(dialog.getByLabel('Gain (dB)')).toHaveValue('-12');
});

test('an input that cannot be opened is said so', async ({ page }) => {
  await fakeInput(page, true);
  await start(page);
  const dialog = await openLineIn(page);
  await dialog.getByRole('button', { name: 'Start' }).click();
  await expect(dialog.getByRole('alert')).toContainText('The input could not be opened: Permission denied');
  await expect(dialog.getByRole('button', { name: 'Start' })).toBeVisible();
});
