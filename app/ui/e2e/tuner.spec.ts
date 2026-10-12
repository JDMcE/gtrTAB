import { expect, test, type Page } from '@playwright/test';
import { start } from './app';

// The tuner, which the plan's eighth phase asks for. The microphone is stood in for by a
// tone made in the page: a sawtooth, which like a string has overtones as well as the note.

/** Puts a tone where the microphone would be, at `window.tone(hertz)`, and two inputs to choose. */
async function fakeInput(page: Page, refuse = false): Promise<void> {
  await page.addInitScript((refuse) => {
    const w = window as unknown as Record<string, unknown>;
    let frequency = 82.407;
    let oscillator: OscillatorNode | undefined;
    w.tone = (hz: number) => {
      frequency = hz;
      if (oscillator) oscillator.frequency.value = hz;
    };
    w.asked = [];
    navigator.mediaDevices.getUserMedia = async (constraints) => {
      (w.asked as unknown[]).push(constraints);
      if (refuse) throw new DOMException('Permission denied', 'NotAllowedError');
      const context = new AudioContext();
      oscillator = context.createOscillator();
      oscillator.type = 'sawtooth';
      oscillator.frequency.value = frequency;
      const gain = context.createGain();
      gain.gain.value = 0.3;
      const out = context.createMediaStreamDestination();
      oscillator.connect(gain).connect(out);
      oscillator.start();
      return out.stream;
    };
    navigator.mediaDevices.enumerateDevices = async () =>
      [
        { kind: 'audioinput', deviceId: 'default', label: 'Default', groupId: 'a' },
        { kind: 'audioinput', deviceId: 'line', label: 'Line in', groupId: 'b' },
        { kind: 'audiooutput', deviceId: 'speakers', label: 'Speakers', groupId: 'c' },
      ] as MediaDeviceInfo[];
  }, refuse);
}

const cents = (hz: number, by: number) => hz * 2 ** (by / 1200);

async function openTuner(page: Page) {
  await page.getByRole('menuitem', { name: 'Tools', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Tuner…', exact: true }).click();
  return page.getByRole('dialog', { name: 'Tuner' });
}

test('the tuner names the note it hears and how far out it is', async ({ page }) => {
  await fakeInput(page);
  await start(page);
  const tuner = await openTuner(page);
  const note = tuner.locator('[data-note]');
  const words = tuner.locator('[data-words]');

  // The low E string, in tune.
  await expect(note).toHaveText('E2');
  await expect(words).toHaveText('In tune');
  await expect(tuner.getByRole('list', { name: 'Strings, from the lowest' }).locator('li.near')).toHaveText('E2');

  // Twenty cents sharp, and then far flat of the A string.
  await page.evaluate((hz) => (window as unknown as { tone: (hz: number) => void }).tone(hz), cents(82.407, 20));
  await expect(words).toHaveText(/^(19|20|21) cents sharp$/);
  await page.evaluate((hz) => (window as unknown as { tone: (hz: number) => void }).tone(hz), cents(110, -70));
  await expect(note).toHaveText('A2');
  await expect(words).toHaveText(/^(69|70|71) cents flat$/);

  // Against the nearest note instead of the strings, the same sound is an A♭ thirty cents up.
  await tuner.getByLabel('The nearest note').check();
  await expect(note).toHaveText('Ab2');
  await expect(words).toHaveText(/^(29|30|31) cents sharp$/);

  // With A at 432 hertz, A2 played at 440's tuning is sharp by 32 cents.
  await page.evaluate((hz) => (window as unknown as { tone: (hz: number) => void }).tone(hz), 110);
  await expect(words).toHaveText('In tune');
  await tuner.getByLabel('A4 (Hz)').fill('432');
  await expect(words).toHaveText(/^(31|32|33) cents sharp$/);

  // Another input is listened to when chosen.
  await tuner.getByLabel('Input').selectOption({ label: 'Line in' });
  await expect.poll(() => page.evaluate(() => JSON.stringify((window as unknown as { asked: unknown[] }).asked.at(-1)))).toContain('"exact":"line"');
  await expect(note).toHaveText('A2');
  await tuner.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(tuner).toBeHidden();
});

test('an input that cannot be opened is said so', async ({ page }) => {
  await fakeInput(page, true);
  await start(page);
  const tuner = await openTuner(page);
  await expect(tuner).toContainText('The input could not be opened: Permission denied');
});
