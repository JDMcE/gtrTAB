import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { applyTheme, loadPrefs } from './lib/prefs';

applyTheme(loadPrefs().theme);

export default mount(App, { target: document.getElementById('app')! });
