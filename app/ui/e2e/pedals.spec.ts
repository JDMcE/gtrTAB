import { readFileSync } from 'node:fs';
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { fret, keys, twoTracks, start } from './app';

// The pedalboard, which the plan's ninth phase asks for: a chain of pedals for each track
// and for the whole mix, with presets, kept with the score and heard in what is played
// and exported.

const board = (page: Page) => page.getByRole('region', { name: 'Pedalboard' });
const pedals = (page: Page) => board(page).locator('li.pedal h3');

async function showPedalboard(page: Page): Promise<void> {
  await page.getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Pedalboard' }).click();
  await expect(board(page)).toBeVisible();
}

const addPedal = (page: Page, name: string) =>
  board(page).getByRole('combobox', { name: 'Add a pedal' }).selectOption({ label: name });

/** What the automated accessibility checks find wrong with the pedalboard. */
async function violations(page: Page): Promise<string[]> {
  const { violations } = await new AxeBuilder({ page }).include('section.pedalboard').analyze();
  return violations.flatMap((v) => v.nodes.map((node) => `${v.id}: ${node.target.join(' ')}`));
}

test('pedals are added, turned, moved and switched off, and heard in the audio exported', async ({ page }, testInfo) => {
  await start(page);
  await fret(page, 0);
  await showPedalboard(page);
  await expect(board(page)).toContainText('No pedals');

  // A pedal goes where its kind usually goes: an amp before a reverb.
  await addPedal(page, 'Reverb');
  await addPedal(page, 'Amp');
  await expect(pedals(page)).toHaveText(['Amp', 'Reverb']);
  expect(await violations(page)).toEqual([]);

  // A knob turned from the keyboard is kept as one step, and undone as one.
  const size = board(page).getByRole('slider', { name: 'Size' });
  await size.focus();
  await keys(page, 'End');
  await expect(size).toHaveAttribute('aria-valuenow', '100');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(size).toHaveAttribute('aria-valuenow', '50');
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect(size).toHaveAttribute('aria-valuenow', '100');

  // Dragged up, a knob goes up; a double click puts it back.
  const gain = board(page).getByRole('slider', { name: 'Gain' });
  const box = (await gain.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 - 40, { steps: 4 });
  await page.mouse.up();
  await expect(gain).toHaveAttribute('aria-valuenow', '75');
  await gain.dblclick();
  await expect(gain).toHaveAttribute('aria-valuenow', '50');

  // Switched off, moved, and taken away.
  const ampOn = board(page).getByRole('button', { name: 'Amp on' });
  await ampOn.click();
  await expect(ampOn).toHaveAttribute('aria-pressed', 'false');
  await board(page).getByRole('button', { name: 'Move Reverb earlier' }).click();
  await expect(pedals(page)).toHaveText(['Reverb', 'Amp']);
  await board(page).getByRole('button', { name: 'Remove Amp' }).click();
  await expect(pedals(page)).toHaveText(['Reverb']);

  // The mix has a chain of its own, put on here from a preset.
  await board(page).getByRole('button', { name: 'Master' }).click();
  await expect(board(page)).toContainText('No pedals: the mix is heard as it is');
  await board(page).getByRole('combobox', { name: 'Put on a preset' }).selectOption({ label: 'Hall' });
  await expect(pedals(page)).toHaveText(['Reverb']);
  await board(page).getByRole('button', { name: 'Guitar' }).click();
  await expect(size).toHaveAttribute('aria-valuenow', '100');

  // A quarter note is half a second, and the reverbs ring on for seconds after it.
  const download = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Audio (WAV)…', exact: true }).click();
  const saved = testInfo.outputPath('reverb.wav');
  await (await download).saveAs(saved);
  const bytes = readFileSync(saved);
  const frames = bytes.readUInt32LE(40) / 4;
  expect(frames).toBeGreaterThan(5 * 44_100);
  let sum = 0;
  for (let frame = 2 * 44_100; frame < 3 * 44_100; frame++) sum += (bytes.readInt16LE(44 + frame * 4) / 32_768) ** 2;
  expect(Math.sqrt(sum / 44_100)).toBeGreaterThan(0.0005);
});

test('a chain saved as a preset is put on another track, and deleted', async ({ page }) => {
  await twoTracks(page);
  await showPedalboard(page);
  // The chain of the steel guitar, from a preset of the app's, saved as one of the user's.
  await board(page).getByRole('combobox', { name: 'Put on a preset' }).selectOption({ label: 'Blues lead' });
  await expect(pedals(page)).toHaveText(['Overdrive', 'Amp', 'Cabinet', 'Delay', 'Reverb']);
  await board(page).getByRole('button', { name: 'Save as a preset' }).click();
  await page.getByRole('dialog').getByLabel('Name').fill('Mine');
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Saved the preset Mine');

  // On the first track, which has no pedals until the preset is put on.
  await page.locator('.tracks .name').first().click();
  await expect(board(page).getByRole('button', { name: 'Guitar', exact: true })).toBeVisible();
  await expect(pedals(page)).toHaveCount(0);
  await board(page).getByRole('combobox', { name: 'Put on a preset' }).selectOption({ label: 'Mine' });
  await expect(pedals(page)).toHaveText(['Overdrive', 'Amp', 'Cabinet', 'Delay', 'Reverb']);

  // The chain is the preset, so the preset can be deleted from here.
  await board(page).getByRole('button', { name: 'Delete the preset Mine' }).click();
  await expect(page.getByRole('status')).toHaveText('Deleted the preset Mine');
  await expect(board(page).getByRole('option', { name: 'Mine' })).toHaveCount(0);
  await expect(pedals(page)).toHaveCount(5);
});
