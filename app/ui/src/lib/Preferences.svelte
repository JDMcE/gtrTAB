<script lang="ts">
  import type { Command } from './commands';
  import { t } from './i18n';
  import { languages, type Language } from './locales';
  import type { Contrast, Paper, Prefs, Theme } from './prefs';
  import { keyOf } from './keys.svelte';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    prefs: Prefs;
    set: <K extends keyof Prefs>(key: K, value: Prefs[K]) => void;
    /** The language chosen when the app started, which is the one in use. */
    languageAtStart: string;
    /** What playback can sound through, each one a choice. */
    sounds: Command[];
    /** Opens the list of keyboard shortcuts, where they are changed. */
    shortcuts: Command;
    palette: Command;
    /** Starts the app again in the language chosen, keeping the score open. */
    onrestart: () => void;
    onreset: () => void;
    onclose: () => void;
  }

  let { prefs, set, languageAtStart, sounds, shortcuts, palette, onrestart, onreset, onclose }: Props = $props();

  let choices = $state<Language[]>([{ code: 'en', name: 'English' }]);
  $effect(() => {
    void languages().then((found) => (choices = found));
  });

  const themes: [Theme, string][] = [
    ['system', t('As the system is')],
    ['light', t('Light')],
    ['dark', t('Dark')],
  ];
  const contrasts: [Contrast, string][] = [
    ['system', t('As the system is')],
    ['normal', t('Normal')],
    ['high', t('High')],
  ];
  const papers: [Paper, string][] = [
    ['theme', t('In the colours of the theme')],
    ['white', t('On white paper, as it is printed')],
  ];
  /** Panels shown by the width of the window, or always, or never. */
  const narrowPalette: [string, string][] = [
    ['auto', t('When the window is narrow')],
    ['true', t('Always')],
    ['false', t('Never')],
  ];
  const inspector: [string, string][] = [
    ['auto', t('When the window is wide enough')],
    ['true', t('Always')],
    ['false', t('Never')],
  ];
  const width = (value: boolean | null) => (value === null ? 'auto' : String(value));
  const fromWidth = (value: string) => (value === 'auto' ? null : value === 'true');
</script>

<Dialog title={t('Preferences')} width="640px" {onclose}>
  <div class="sections">
    <section>
      <h3>{t('Appearance')}</h3>
      <label>
        {t('Theme')}
        <select value={prefs.theme} onchange={(e) => set('theme', e.currentTarget.value as Theme)}>
          {#each themes as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label>
        {t('Contrast')}
        <select value={prefs.contrast} onchange={(e) => set('contrast', e.currentTarget.value as Contrast)}>
          {#each contrasts as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label>
        {t('The score')}
        <select value={prefs.paper} onchange={(e) => set('paper', e.currentTarget.value as Paper)}>
          {#each papers as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label>
        {t('Language')}
        <select value={prefs.language} onchange={(e) => set('language', e.currentTarget.value)}>
          <option value="system">{t('As the system is')}</option>
          {#each choices as language (language.code)}<option value={language.code}>{language.name}</option>{/each}
        </select>
      </label>
      {#if prefs.language !== languageAtStart}
        <p class="note" role="status">
          {t('The interface changes language when gtrTab starts again.')}
          <button type="button" class="btn" onclick={onrestart}>{t('Start again now')}</button>
        </p>
      {/if}
    </section>

    <section>
      <h3>{t('Panels')}</h3>
      <label>
        {t('Narrow palette')}
        <select value={width(prefs.compact)} onchange={(e) => set('compact', fromWidth(e.currentTarget.value))}>
          {#each narrowPalette as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label>
        {t('Inspector')}
        <select value={width(prefs.inspector)} onchange={(e) => set('inspector', fromWidth(e.currentTarget.value))}>
          {#each inspector as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label class="check">
        <input type="checkbox" checked={prefs.tracks} onchange={(e) => set('tracks', e.currentTarget.checked)} />
        {t('Track pane under the score')}
      </label>
      <label class="check">
        <input type="checkbox" checked={prefs.beatLights} onchange={(e) => set('beatLights', e.currentTarget.checked)} />
        {t('Visual metronome in the transport bar')}
      </label>
    </section>

    <section>
      <h3>{t('Sound')}</h3>
      <div class="choices" role="radiogroup" aria-label={t('Playback sounds through')}>
        {#each sounds as sound (sound.label)}
          <label class="check">
            <input type="radio" name="sound" checked={sound.active?.()} onchange={() => sound.run()} />
            {sound.label}
          </label>
        {/each}
      </div>
    </section>

    <section>
      <h3>{t('Keyboard')}</h3>
      <p class="plain">
        {t('Any command in the menus can be given a key of your choosing, or found by name in the command palette.')}
      </p>
      <div class="buttons">
        <button type="button" class="btn" onclick={() => shortcuts.run()}>{t('Keyboard shortcuts…')}</button>
        <button type="button" class="btn" onclick={() => palette.run()}>
          {t('Command palette')}{#if keyOf(palette)}&ensp;<kbd>{keyOf(palette)}</kbd>{/if}
        </button>
      </div>
    </section>
  </div>

  {#snippet footer()}
    <button type="button" class="btn remove" onclick={onreset}>{t('Reset preferences')}</button>
    <button type="button" class="btn primary" onclick={onclose}>{t('Done')}</button>
  {/snippet}
</Dialog>

<style>
  .sections {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: var(--space-2) var(--space-6);
  }

  h3 {
    margin: 0 0 var(--space-3);
    color: var(--accent);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  section {
    margin-bottom: var(--space-4);
  }

  .note {
    display: grid;
    gap: var(--space-3);
    justify-items: start;
    margin: 0 0 var(--space-4);
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .plain {
    margin: 0 0 var(--space-4);
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .choices :global(label.check) {
    margin-bottom: var(--space-3);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
</style>
