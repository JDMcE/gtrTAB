// Batch conversion: many scores, of any format the app opens, written in one format.

/** What scores can be converted to, as File > Batch convert lists them. */
export type BatchTarget = 'gtrtab' | 'guitar_pro' | 'musicxml' | 'midi' | 'ascii' | 'pdf' | 'png' | 'svg';

/** How each target ends a file's name. */
export const TARGET_EXTENSIONS: Record<BatchTarget, string> = {
  gtrtab: 'gtrtab',
  guitar_pro: 'gp',
  musicxml: 'musicxml',
  midi: 'mid',
  ascii: 'txt',
  pdf: 'pdf',
  png: 'png',
  svg: 'svg',
};

/** A score converted, or why it could not be. */
export interface Converted {
  /** The name of the file it came from. */
  from: string;
  /** The name it was written under; `null` if it could not be converted. */
  name: string | null;
  /** What was left out reading it and writing it, or why it failed. */
  note: string;
  failed: boolean;
}

const stem = (name: string) => name.replace(/\.[^./\\]*$/, '') || name;

/**
 * The name a converted file is written under: the name it came from with the target's
 * ending, numbered as `riff (2).pdf` if a file already written, or already where it is
 * going, has that name. Names are compared as a file system that ignores case does.
 * The name chosen is added to `taken`.
 */
export function outputName(from: string, extension: string, taken: Set<string>): string {
  const base = stem(from);
  let name = `${base}.${extension}`;
  for (let n = 2; taken.has(name.toLowerCase()); n++) name = `${base} (${n}).${extension}`;
  taken.add(name.toLowerCase());
  return name;
}

/** What was left out of a score converted, from the reports of its reading and its writing. */
export function leftOut(imported: string, exported: string): string {
  return [imported && `not imported: ${imported}`, exported && `not exported: ${exported}`]
    .filter(Boolean)
    .join('; ');
}
