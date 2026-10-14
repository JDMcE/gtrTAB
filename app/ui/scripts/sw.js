// The service worker of gtrTab's web app, written into each build by scripts/offline.mjs.
// The files of the build are kept when it is installed, so that the app opens with no
// connection; a new build is taken up when every window of the old one has closed. Other
// files of the app's own, such as the SoundFont, are kept the first time they are fetched.

const VERSION = '__VERSION__';
const FILES = __FILES__;
const BUILD = `gtrtab-${VERSION}`;
const FETCHED = 'gtrtab-fetched';

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(BUILD).then((cache) => cache.addAll(FILES)));
});

self.addEventListener('activate', (event) => {
  // The builds before this one are no longer needed; what was fetched is kept.
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== BUILD && k !== FETCHED).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== 'GET' || url.origin !== self.location.origin) return;
  // The page itself comes from the build kept, whatever is asked for after the app's address.
  if (request.mode === 'navigate') {
    event.respondWith(
      caches.match('./', { cacheName: BUILD, ignoreVary: true }).then((page) => page ?? fetch(request)),
    );
    return;
  }
  event.respondWith(
    // The files were kept as fetched without an origin, and are the same whoever asks.
    caches.match(request, { ignoreSearch: true, ignoreVary: true }).then(
      (kept) =>
        kept ??
        fetch(request).then((response) => {
          // A part of a file, as a player asks for, is not the file.
          if (response.ok && response.status === 200) {
            const copy = response.clone();
            void caches.open(FETCHED).then((cache) => cache.put(request, copy));
          }
          return response;
        }),
    ),
  );
});
