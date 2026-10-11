// Downloads the SoundFont the app plays through by default, GeneralUser GS by S. Christian
// Collins, with its licence. It is too big to keep in the repository, so it is fetched from
// a fixed revision of its own and checked against a known hash.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REVISION = '684543d5e5efaef08d02be50dcda8d552478fa60';
const SOURCE = `https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/${REVISION}`;
const FILES = [
  {
    from: 'GeneralUser-GS.sf2',
    to: 'GeneralUser-GS.sf2',
    sha256: '9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe',
  },
  { from: 'documentation/LICENSE.txt', to: 'GeneralUser-GS.LICENSE.txt' },
];

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'public', 'soundfonts');
mkdirSync(out, { recursive: true });
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

for (const file of FILES) {
  const path = join(out, file.to);
  if (existsSync(path) && (!file.sha256 || sha256(readFileSync(path)) === file.sha256)) continue;
  console.log(`Fetching ${file.from}`);
  const response = await fetch(`${SOURCE}/${encodeURI(file.from)}`);
  if (!response.ok) throw new Error(`${file.from}: ${response.status} ${response.statusText}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  if (file.sha256 && sha256(bytes) !== file.sha256) {
    throw new Error(`${file.from} is not the file expected: its hash is ${sha256(bytes)}`);
  }
  writeFileSync(path, bytes);
}
