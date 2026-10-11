// Builds the synth for the AudioWorklet and puts it beside the rest of the WASM core.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const target = 'wasm32-unknown-unknown';

// Always optimised: the synth has to keep up with the sound card, also while developing.
execFileSync('cargo', ['build', '--package', 'gtr-worklet', '--target', target, '--release'], {
  cwd: root,
  stdio: 'inherit',
});
const out = join(root, 'app', 'ui', 'src', 'wasm');
mkdirSync(out, { recursive: true });
copyFileSync(
  join(root, 'target', target, 'release', 'gtr_worklet.wasm'),
  join(out, 'gtr_worklet.wasm'),
);
