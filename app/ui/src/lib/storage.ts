// What the app keeps in the browser's database: files too big for localStorage, such as a
// SoundFont the user picked and the recording of work not saved.

const DATABASE = 'gtrtab';
/** Each version adds stores; those already there are kept as they are. */
const VERSION = 2;
const STORES = ['soundfont', 'recording'] as const;

export type Store = (typeof STORES)[number];

/** Runs one request against a store of the database. */
export function request<T>(
  name: Store,
  mode: IDBTransactionMode,
  make: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const opening = indexedDB.open(DATABASE, VERSION);
    opening.onupgradeneeded = () => {
      for (const store of STORES) {
        if (!opening.result.objectStoreNames.contains(store)) opening.result.createObjectStore(store);
      }
    };
    opening.onerror = () => reject(opening.error);
    opening.onsuccess = () => {
      const database = opening.result;
      const made = make(database.transaction(name, mode).objectStore(name));
      made.onerror = () => reject(made.error);
      made.onsuccess = () => resolve(made.result);
      made.transaction!.oncomplete = () => database.close();
    };
  });
}
