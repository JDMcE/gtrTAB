import { mount } from 'svelte';
import './app.css';
import { startLanguage } from './lib/locales';
import { applyLook, applyTheme, loadPrefs } from './lib/prefs';

const prefs = loadPrefs();
applyTheme(prefs.theme);
applyLook(prefs.contrast, prefs.paper);

// Every text is translated where it is made, so the language is chosen before the app,
// and the modules that make texts, are loaded.
void startLanguage(prefs.language)
  .then(() => import('./App.svelte'))
  .then(({ default: App }) => mount(App, { target: document.getElementById('app')! }));

// The web app keeps its files for when there is no connection; the desktop app has them on disk.
if (import.meta.env.PROD && 'serviceWorker' in navigator && !('__TAURI_INTERNALS__' in window)) {
  void navigator.serviceWorker.register('sw.js').catch(() => {
    // Without it the app still works while there is a connection.
  });
}
