// Where the samples the synth plays from come from: the SoundFont that ships with the app,
// or one the user has picked, which is kept in the browser's storage for next time.

import { request } from './storage';

/** The bank that ships with the app. `npm run soundfont` fetches it; see scripts/fetch-soundfont.mjs. */
export const DEFAULT_SOUNDFONT = { name: 'GeneralUser GS', file: 'soundfonts/GeneralUser-GS.sf2' };

export interface StoredSoundFont {
  name: string;
  bytes: ArrayBuffer;
}

/** The bank that ships with the app, or `null` if this build was made without it. */
export async function fetchDefaultSoundFont(): Promise<ArrayBuffer | null> {
  try {
    const response = await fetch(import.meta.env.BASE_URL + DEFAULT_SOUNDFONT.file);
    // A dev server answers for a missing file with the page itself.
    const isPage = response.headers.get('content-type')?.includes('text/html');
    return response.ok && !isPage ? await response.arrayBuffer() : null;
  } catch {
    return null;
  }
}

/** Lets the user pick a SoundFont file. Resolves to `null` if they cancel. */
export function pickSoundFont(): Promise<StoredSoundFont | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.sf2';
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return resolve(null);
      file.arrayBuffer().then((bytes) => resolve({ name: file.name, bytes }), reject);
    };
    input.oncancel = () => resolve(null);
    input.click();
  });
}

const KEY = 'custom';

/** Keeps the user's SoundFont for the next time the app starts. */
export async function storeSoundFont(font: StoredSoundFont): Promise<void> {
  try {
    await request('soundfont', 'readwrite', (s) => s.put(font, KEY));
  } catch {
    // Without storage the bank still plays now; it only has to be picked again next time.
  }
}

/** The SoundFont the user picked before, if it is still there. */
export async function storedSoundFont(): Promise<StoredSoundFont | null> {
  try {
    return (await request<StoredSoundFont | undefined>('soundfont', 'readonly', (s) => s.get(KEY))) ?? null;
  } catch {
    return null;
  }
}
