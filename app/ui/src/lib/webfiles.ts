// Files on the user's own disk in a browser that lets a page keep hold of them (the File
// System Access API, in Chromium-based browsers): a score opened can be saved back to where
// it came from and opened again from the recent files, as on desktop. Each file or folder
// picked is kept in the browser's database by a path of its own, `web:<id>/<name>`, which
// stands for it where the desktop has a path on disk.

import { t } from './i18n';
import { request } from './storage';

interface PickerType {
  description: string;
  accept: Record<string, string[]>;
}

type Permission = 'granted' | 'denied' | 'prompt';

/** A file or folder picked, as the browser hands it over. */
interface Handle {
  kind: 'file' | 'directory';
  name: string;
  isSameEntry(other: Handle): Promise<boolean>;
  queryPermission(options: { mode: 'read' | 'readwrite' }): Promise<Permission>;
  requestPermission(options: { mode: 'read' | 'readwrite' }): Promise<Permission>;
}

interface FileHandle extends Handle {
  kind: 'file';
  getFile(): Promise<File>;
  createWritable(): Promise<{ write(data: BlobPart): Promise<void>; close(): Promise<void> }>;
}

interface FolderHandle extends Handle {
  kind: 'directory';
  getFileHandle(name: string, options?: { create?: boolean }): Promise<FileHandle>;
}

interface Pickers {
  showOpenFilePicker?: (options: { multiple?: boolean; types?: PickerType[] }) => Promise<FileHandle[]>;
  showSaveFilePicker?: (options: { suggestedName?: string; types?: PickerType[] }) => Promise<FileHandle>;
  showDirectoryPicker?: (options: { mode?: 'read' | 'readwrite' }) => Promise<FolderHandle>;
}

const pickers = window as unknown as Pickers;

/** Whether files can be picked and kept hold of, rather than only uploaded and downloaded. */
export const canOpen = () => typeof pickers.showOpenFilePicker === 'function';
export const canSave = () => typeof pickers.showSaveFilePicker === 'function';
export const canPickFolder = () => typeof pickers.showDirectoryPicker === 'function';

export const isWebPath = (path: string) => path.startsWith('web:');

/** The name of a file a path stands for. */
export const webName = (path: string) => path.slice(path.indexOf('/') + 1);

/** A picker the user closed without choosing: not a failure. */
const cancelled = (e: unknown) => e instanceof DOMException && e.name === 'AbortError';

const types = (kind: string, extensions: string[]): PickerType[] => [
  { description: kind, accept: { 'application/octet-stream': extensions.map((e) => `.${e}`) } },
];

/** Keeps hold of a file or folder, and returns the path it now has: the one it had if it was kept before. */
async function keep(handle: Handle): Promise<string> {
  const kept = await request<IDBValidKey[]>('handles', 'readonly', (s) => s.getAllKeys());
  for (const key of kept) {
    const other = await request<Handle | undefined>('handles', 'readonly', (s) => s.get(key));
    if (other && other.kind === handle.kind && (await other.isSameEntry(handle))) return `web:${String(key)}/${handle.name}`;
  }
  const id = crypto.randomUUID();
  await request('handles', 'readwrite', (s) => s.put(handle, id));
  return `web:${id}/${handle.name}`;
}

/** The file or folder a path stands for, with leave to read it or write it, asked for if need be. */
async function kept<T extends Handle>(path: string, mode: 'read' | 'readwrite'): Promise<T> {
  const id = path.slice('web:'.length, path.indexOf('/'));
  const handle = await request<T | undefined>('handles', 'readonly', (s) => s.get(id));
  if (!handle) throw new Error(t('{name} is no longer at hand.', { name: webName(path) }));
  if ((await handle.queryPermission({ mode })) !== 'granted' && (await handle.requestPermission({ mode })) !== 'granted') {
    throw new Error(
      mode === 'read'
        ? t('gtrTab was not let read {name}.', { name: handle.name })
        : t('gtrTab was not let write {name}.', { name: handle.name }),
    );
  }
  return handle;
}

/** Lets go of a file or folder kept hold of. */
export async function forget(path: string): Promise<void> {
  try {
    await request('handles', 'readwrite', (s) => s.delete(path.slice('web:'.length, path.indexOf('/'))));
  } catch {
    // It only takes up a little room.
  }
}

/**
 * Lets the user pick files. With `hold`, each is kept hold of and has a path to be opened
 * again by; without, its path is `null`. Resolves to none if they cancel.
 */
export async function pick(
  kind: string,
  extensions: string[],
  multiple: boolean,
  hold: boolean,
): Promise<{ name: string; path: string | null; bytes: Uint8Array }[]> {
  let handles: FileHandle[];
  try {
    handles = await pickers.showOpenFilePicker!({ multiple, types: types(kind, extensions) });
  } catch (e) {
    if (cancelled(e)) return [];
    throw e;
  }
  return Promise.all(
    handles.map(async (handle) => ({
      name: handle.name,
      path: hold ? await keep(handle) : null,
      bytes: new Uint8Array(await (await handle.getFile()).arrayBuffer()),
    })),
  );
}

/** Reads a file kept hold of. */
export async function read(path: string): Promise<{ name: string; path: string; bytes: Uint8Array }> {
  const handle = await kept<FileHandle>(path, 'read');
  return { name: handle.name, path, bytes: new Uint8Array(await (await handle.getFile()).arrayBuffer()) };
}

async function write(handle: FileHandle, bytes: Uint8Array): Promise<void> {
  const writable = await handle.createWritable();
  await writable.write(bytes as BlobPart);
  await writable.close();
}

/**
 * Writes a file: to the one `path` stands for, or with `null` to one the user picks, which
 * with `hold` is then kept hold of. Resolves to its name and path (`null` if not kept
 * hold of), or to `null` if the user cancelled.
 */
export async function save(
  name: string,
  path: string | null,
  bytes: Uint8Array,
  kind: string,
  hold = true,
): Promise<{ name: string; path: string | null } | null> {
  if (path !== null) {
    await write(await kept<FileHandle>(path, 'readwrite'), bytes);
    return { name: webName(path), path };
  }
  let handle: FileHandle;
  try {
    const extension = name.split('.').pop() ?? '';
    handle = await pickers.showSaveFilePicker!({ suggestedName: name, types: types(kind, [extension]) });
  } catch (e) {
    if (cancelled(e)) return null;
    throw e;
  }
  await write(handle, bytes);
  return { name: handle.name, path: hold ? await keep(handle) : null };
}

/** Lets the user pick a folder to write files into, and keeps hold of it. `null` if they cancel. */
export async function pickFolder(): Promise<string | null> {
  try {
    return await keep(await pickers.showDirectoryPicker!({ mode: 'readwrite' }));
  } catch (e) {
    if (cancelled(e)) return null;
    throw e;
  }
}

/** Whether a folder kept hold of has a file of this name, and if not, writes it there. */
export async function writeNew(folder: string, name: string, bytes: Uint8Array): Promise<boolean> {
  const handle = await kept<FolderHandle>(folder, 'readwrite');
  try {
    await handle.getFileHandle(name);
    return false;
  } catch (e) {
    if (!(e instanceof DOMException && e.name === 'NotFoundError')) throw e;
  }
  await write(await handle.getFileHandle(name, { create: true }), bytes);
  return true;
}

interface LaunchQueue {
  setConsumer(consumer: (params: { files: FileHandle[] }) => void): void;
}

/**
 * Has `open` called with each file the system opens with the app, once it is installed
 * and a score is opened with it from the desktop.
 */
export function onLaunch(open: (file: { name: string; path: string; bytes: Uint8Array }) => void): void {
  const queue = (window as unknown as { launchQueue?: LaunchQueue }).launchQueue;
  queue?.setConsumer(async ({ files }) => {
    for (const handle of files) {
      open({ name: handle.name, path: await keep(handle), bytes: new Uint8Array(await (await handle.getFile()).arrayBuffer()) });
    }
  });
}
