// Opening and saving score files: native dialogs inside Tauri, browser fallbacks on the web.

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
  /** Full path on desktop; `null` on the web, where files cannot be reopened by path. */
  path: string | null;
  bytes: Uint8Array;
}

export const isTauri = '__TAURI_INTERNALS__' in window;

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

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

/** Lets the user pick a file. Resolves to `null` if they cancel. */
export async function openFile(): Promise<OpenedFile | null> {
  const extensions = [EXTENSION, ...IMPORT_EXTENSIONS];
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({
      multiple: false,
      filters: [{ name: 'Scores', extensions }],
    });
    return path === null ? null : readPath(path);
  }

  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = extensions.map((e) => `.${e}`).join(',');
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return resolve(null);
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        resolve({ name: file.name, path: null, bytes });
      } catch (e) {
        reject(e);
      }
    };
    input.oncancel = () => resolve(null);
    input.click();
  });
}

/** Reads a file by path. Desktop only. */
export async function readPath(path: string): Promise<OpenedFile> {
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

/** Paths of recently used files, newest first. Always empty on the web. */
export function recentFiles(): string[] {
  return isTauri ? (read<string[]>(RECENT_KEY) ?? []) : [];
}

export function addRecentFile(path: string): string[] {
  const list = [path, ...recentFiles().filter((p) => p !== path)].slice(0, MAX_RECENT);
  write(RECENT_KEY, list);
  return list;
}

export function removeRecentFile(path: string): string[] {
  const list = recentFiles().filter((p) => p !== path);
  write(RECENT_KEY, list);
  return list;
}

export interface Autosave {
  name: string;
  path: string | null;
  bytes: Uint8Array;
}

/** Stores unsaved work so it survives a crash or closed tab; `null` clears it. */
export function writeAutosave(autosave: Autosave | null): void {
  if (autosave === null) return write(AUTOSAVE_KEY, null);
  let binary = '';
  for (const byte of autosave.bytes) binary += String.fromCharCode(byte);
  write(AUTOSAVE_KEY, { name: autosave.name, path: autosave.path, data: btoa(binary) });
}

export function readAutosave(): Autosave | null {
  const stored = read<{ name: string; path: string | null; data: string }>(AUTOSAVE_KEY);
  if (!stored) return null;
  try {
    const bytes = Uint8Array.from(atob(stored.data), (c) => c.charCodeAt(0));
    return { name: stored.name, path: stored.path, bytes };
  } catch {
    return null;
  }
}
