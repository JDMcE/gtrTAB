// Reads every Guitar Pro and MusicXML file of the corpus with alphaTab and writes down,
// for each track and bar, the pitches of the notes it finds. `alphatab.json` is what it
// wrote with alphaTab 1.8.4, and `crates/gtr-io/tests/reference.rs` holds the importers
// to it. To write it again, in a directory of its own:
//
//   npm install @coderline/alphatab@1.8.4
//   node alphatab.mjs <repository>/tests/corpus/alphatab alphatab.json
import * as alphaTab from '@coderline/alphatab';
import fs from 'node:fs';
import path from 'node:path';

const root = process.argv[2];
const out = {};
function walk(dir) {
  for (const name of fs.readdirSync(dir).sort()) {
    const full = path.join(dir, name);
    if (fs.statSync(full).isDirectory()) walk(full);
    else if (/\.(gp3|gp4|gp5|gpx|gp|xml|musicxml|mxl)$/.test(name)) read(full);
  }
}
function read(file) {
  const key = path.relative(root, file).split(path.sep).join('/');
  let score;
  try {
    score = alphaTab.importer.ScoreLoader.loadScoreFromBytes(new Uint8Array(fs.readFileSync(file)));
  } catch (e) {
    out[key] = { error: String(e.message ?? e).slice(0, 80) };
    return;
  }
  const tracks = score.tracks.map((track) => {
    const bars = score.masterBars.map(() => []);
    for (const staff of track.staves) {
      staff.bars.forEach((bar, index) => {
        for (const voice of bar.voices) for (const beat of voice.beats) for (const note of beat.notes) {
          let pitch;
          if (note.isPercussion) {
            // A piece of the track's own kit by its place in it, or else its number.
            const kit = track.percussionArticulations;
            const piece = kit && kit.length ? kit[note.percussionArticulation] : undefined;
            pitch = piece ? piece.outputMidiNumber : note.percussionArticulation;
          } else pitch = note.realValueWithoutHarmonic;
          bars[index].push(pitch);
        }
      });
    }
    return bars.map((bar) => bar.sort((a, b) => a - b));
  });
  out[key] = { bars: score.masterBars.length, tracks };
}
walk(root);
fs.writeFileSync(process.argv[3], JSON.stringify(out));
console.log(Object.keys(out).length, 'files', Object.values(out).filter((v) => v.error).length, 'errors');
