// The web app's service worker, written at the end of a build with the list of every file
// the build made, so that the app opens and works with no connection once it has been
// opened with one. Tauri serves the app from the disk and has no need of it.

import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

/** Files in the build that are not kept ahead of use: licences, and anything this big. */
const LEFT_OUT = /\.(txt|map)$/;
const BIGGEST = 8 * 1024 * 1024;

function* files(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path);
    else yield path;
  }
}

/** A Vite plugin that writes `sw.js` into the build. */
export function offline() {
  let out = 'dist';
  return {
    name: 'gtrtab-offline',
    apply: 'build',
    configResolved(config) {
      out = config.build.outDir;
    },
    closeBundle() {
      const hash = createHash('sha256');
      const kept = [];
      for (const path of files(out)) {
        const name = relative(out, path).split(sep).join('/');
        if (name === 'sw.js' || LEFT_OUT.test(name) || statSync(path).size > BIGGEST) continue;
        hash.update(name).update(readFileSync(path));
        kept.push(name === 'index.html' ? './' : name);
      }
      const version = hash.digest('hex').slice(0, 16);
      const worker = readFileSync(new URL('./sw.js', import.meta.url), 'utf8')
        .replace('__VERSION__', version)
        .replace('__FILES__', JSON.stringify(kept.sort()));
      writeFileSync(join(out, 'sw.js'), worker);
    },
  };
}
