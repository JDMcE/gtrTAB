// Opening and saving score files: native dialogs inside Tauri; on the web, the files the
// browser lets the page keep hold of where it can (`webfiles.ts`), and otherwise uploads
// and downloads.

import { outputName } from './batch';
import { request } from './storage';
import * as web from './webfiles';

export const EXTENSION = 'gtrtab';
/** Files of other programs that can be opened: Guitar Pro, MusicXML, MIDI and plain-text tabs. */
export const IMPORT_EXTENSIONS = [
  'gp', 'gpx', 'gp5', 'gp4', 'gp3',
  'musicxml', 'mxl', 'xml',
  'mid', 'midi',
  'txt', 'tab',
];

export interface OpenedFile {
  name: string;
  /**
   * Full path on desktop, or on the web the path a file kept hold of has; `null` for a
   * file that cannot be opened again by path.
   */
  path: string | null;
  bytes: Uint8Array;
}

export const isTauri = '__TAURI_INTERNALS__' in window;

/** The name of the file a path stands for, on desktop or on the web. */
export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Sets the browser tab title or, inside Tauri, the window title. */
export async function setWindowTitle(title: string): Promise<void> {
  document.title = title;
  if (isTauri) {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().setTitle(title);
  }
}

/** Asks a yes/no question. */
export async function confirmDialog(message: string): Promise<boolean> {
  if (isTauri) {
    const { ask } = await import('@tauri-apps/plugin-dialog');
    return ask(message, { title: 'gtrTab', kind: 'warning' });
  }
  return window.confirm(message);
}

/** Recordings an audio track can be made from: what browsers and the desktop's webviews can decode. */
export const AUDIO_EXTENSIONS = ['mp3', 'ogg', 'oga', 'opus', 'wav', 'flac', 'm4a', 'aac', 'webm'];

/** Lets the user pick a score. Resolves to `null` if they cancel. */
export async function openFile(): Promise<OpenedFile | null> {
  return (await pickFiles('Scores', [EXTENSION, ...IMPORT_EXTENSIONS], false, true))[0] ?? null;
}

/** Lets the user pick any number of scores. Resolves to none if they cancel. */
export function openFiles(): Promise<OpenedFile[]> {
  return pickFiles('Scores', [EXTENSION, ...IMPORT_EXTENSIONS], true, false);
}

/** Lets the user pick a recording. Resolves to `null` if they cancel. */
export async function openAudioFile(): Promise<OpenedFile | null> {
  return (await pickFiles('Recordings', AUDIO_EXTENSIONS, false, false))[0] ?? null;
}

/** Lets the user pick files; with `hold`, ones that can be opened again by their path. */
async function pickFiles(kind: string, extensions: string[], multiple: boolean, hold: boolean): Promise<OpenedFile[]> {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked: string | string[] | null = await open({ multiple, filters: [{ name: kind, extensions }] });
    if (picked === null) return [];
    return Promise.all((Array.isArray(picked) ? picked : [picked]).map(readPath));
  }
  if (web.canOpen()) return web.pick(kind, extensions, multiple, hold);

  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = multiple;
    input.accept = extensions.map((e) => `.${e}`).join(',');
    input.onchange = async () => {
      try {
        resolve(
          await Promise.all(
            [...(input.files ?? [])].map(async (file) => ({
              name: file.name,
              path: null,
              bytes: new Uint8Array(await file.arrayBuffer()),
            })),
          ),
        );
      } catch (e) {
        reject(e);
      }
    };
    input.oncancel = () => resolve([]);
    input.click();
  });
}

/** Whether files can be written into a folder the user picks, rather than downloaded. */
export const canPickFolder = () => isTauri || web.canPickFolder();

/** Lets the user pick a folder to write files into; `null` if they cancel. */
export async function pickFolder(): Promise<string | null> {
  if (!isTauri) return web.pickFolder();
  const { open } = await import('@tauri-apps/plugin-dialog');
  return open({ directory: true, multiple: false });
}

/**
 * Writes a file into a folder picked, under its name or, if a file there or one written
 * already has it, numbered as `riff (2).pdf`. Resolves to the name written.
 */
export async function writeInto(folder: string, from: string, extension: string, bytes: Uint8Array, taken: Set<string>) {
  if (web.isWebPath(folder)) {
    for (;;) {
      const name = outputName(from, extension, taken);
      if (await web.writeNew(folder, name, bytes)) return name;
    }
  }
  const [{ exists, writeFile }, { join }] = await Promise.all([
    import('@tauri-apps/plugin-fs'),
    import('@tauri-apps/api/path'),
  ]);
  for (;;) {
    const name = outputName(from, extension, taken);
    const path = await join(folder, name);
    if (await exists(path)) continue;
    await writeFile(path, bytes);
    return name;
  }
}

/** Reads a file by path: on disk on desktop, or a file kept hold of on the web. */
export async function readPath(path: string): Promise<OpenedFile> {
  if (web.isWebPath(path)) return web.read(path);
  const { readFile } = await import('@tauri-apps/plugin-fs');
  return { name: baseName(path), path, bytes: await readFile(path) };
}

/**
 * Saves a score. On desktop it writes to `path`, asking where if `path` is `null`; on the
 * web it downloads the file. Resolves to the name and path written, or `null` if cancelled.
 */
export async function saveFile(
  name: string,
  path: string | null,
  bytes: Uint8Array,
): Promise<{ name: string; path: string | null } | null> {
  if (isTauri) {
    if (path === null) {
      const { save } = await import('@tauri-apps/plugin-dialog');
      path = await save({
        defaultPath: name,
        filters: [{ name: 'gtrTab score', extensions: [EXTENSION] }],
      });
      if (path === null) return null;
    }
    const { writeFile } = await import('@tauri-apps/plugin-fs');
    await writeFile(path, bytes);
    return { name: baseName(path), path };
  }
  if (path !== null || web.canSave()) return web.save(name, path, bytes, 'gtrTab score');

  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'application/zip' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
  return { name, path: null };
}

/**
 * Saves something made from the score, such as its sound, under a name the user chooses:
 * through the save dialog on desktop, and as a download on the web. `kind` names the sort
 * of file for the dialog and `type` is its media type. Resolves to the name written, or
 * `null` if the user cancelled.
 */
export async function exportFile(
  name: string,
  bytes: Uint8Array,
  kind: string,
  type: string,
): Promise<string | null> {
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({
      defaultPath: name,
      filters: [{ name: kind, extensions: [name.split('.').pop() ?? ''] }],
    });
    if (path === null) return null;
    const { writeFile } = await import('@tauri-apps/plugin-fs');
    await writeFile(path, bytes);
    return baseName(path);
  }
  if (web.canSave()) return (await web.save(name, null, bytes, kind, false))?.name ?? null;
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
  return name;
}

// --- Recent files and autosave, kept in localStorage ---

const RECENT_KEY = 'gtrtab.recent';
const AUTOSAVE_KEY = 'gtrtab.autosave';
const MAX_RECENT = 8;

function read<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as T);
  } catch {
    return null;
  }
}

function write(key: string, value: unknown): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage may be full or unavailable; neither feature is essential.
  }
}

/** Paths of recently used files, newest first; on the web, only where files can be kept hold of. */
export function recentFiles(): string[] {
  return isTauri || web.canOpen() ? (read<string[]>(RECENT_KEY) ?? []) : [];
}

export function addRecentFile(path: string): string[] {
  const all = [path, ...recentFiles().filter((p) => p !== path)];
  // A file that falls off the end is no longer kept hold of.
  for (const dropped of all.slice(MAX_RECENT)) if (web.isWebPath(dropped)) void web.forget(dropped);
  const list = all.slice(0, MAX_RECENT);
  write(RECENT_KEY, list);
  return list;
}

export function removeRecentFile(path: string): string[] {
  const list = recentFiles().filter((p) => p !== path);
  if (web.isWebPath(path)) void web.forget(path);
  write(RECENT_KEY, list);
  return list;
}

export interface Autosave {
  name: string;
  path: string | null;
  bytes: Uint8Array;
  /** Kept only for the app to start again with, and saved already: nothing to recover. */
  saved?: boolean;
}

/** Stores unsaved work so it survives a crash or closed tab; `null` clears it. */
export function writeAutosave(autosave: Autosave | null): void {
  if (autosave === null) return write(AUTOSAVE_KEY, null);
  let binary = '';
  for (const byte of autosave.bytes) binary += String.fromCharCode(byte);
  write(AUTOSAVE_KEY, { name: autosave.name, path: autosave.path, data: btoa(binary), saved: autosave.saved });
}

export function readAutosave(): Autosave | null {
  const stored = read<{ name: string; path: string | null; data: string; saved?: boolean }>(AUTOSAVE_KEY);
  if (!stored) return null;
  try {
    const bytes = Uint8Array.from(atob(stored.data), (c) => c.charCodeAt(0));
    return { name: stored.name, path: stored.path, bytes, saved: stored.saved };
  } catch {
    return null;
  }
}

export interface KeptRecording {
  name: string;
  bytes: Uint8Array;
}

/**
 * Keeps the recording of work not saved beside the copy of it, or with `null` lets it go.
 * It is too big for where the copy is kept, so it goes in the browser's database.
 */
export async function keepAutosaveRecording(recording: KeptRecording | null): Promise<void> {
  try {
    if (recording) await request('recording', 'readwrite', (s) => s.put(recording, 'autosave'));
    else await request('recording', 'readwrite', (s) => s.delete('autosave'));
  } catch {
    // Recovered work then asks for the file of its recording, as it would have anyway.
  }
}

/** The recording kept beside the copy of work not saved, if there is one. */
export async function autosaveRecording(): Promise<KeptRecording | null> {
  try {
    return (await request<KeptRecording | undefined>('recording', 'readonly', (s) => s.get('autosave'))) ?? null;
  } catch {
    return null;
  }
}

const REOPEN_KEY = 'gtrtab.reopen';

/** Has the app open a saved file again when it starts next, in this window. */
export function reopenAfterRestart(path: string): void {
  try {
    sessionStorage.setItem(REOPEN_KEY, path);
  } catch {
    // Without storage the app starts with a new score, and the file is in the recent list.
  }
}

/** The file to open again as the app starts, if one was left, which is then forgotten. */
export function takeReopenAfterRestart(): string | null {
  try {
    const path = sessionStorage.getItem(REOPEN_KEY);
    sessionStorage.removeItem(REOPEN_KEY);
    return path;
  } catch {
    return null;
  }
}
