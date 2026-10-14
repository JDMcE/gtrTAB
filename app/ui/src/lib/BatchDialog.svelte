<script lang="ts">
  import { TARGET_EXTENSIONS, type Converted, type BatchTarget } from './batch';
  import { canPickFolder, openFiles, type OpenedFile } from './files';
  import { t } from './i18n';
  import Dialog from './ui/Dialog.svelte';
  import Icon from './ui/Icon.svelte';

  interface Props {
    /**
     * Converts the scores and writes them out, telling of each as it is done. Resolves to
     * what to say once all are, or `null` if the user cancelled before any was written.
     */
    onconvert: (files: OpenedFile[], target: BatchTarget, done: (result: Converted) => void) => Promise<string | null>;
    onclose: () => void;
  }

  let { onconvert, onclose }: Props = $props();

  const TARGET_KEY = 'gtrtab.batchTarget';

  const targets: [BatchTarget, string][] = [
    ['gtrtab', t('gtrTab score')],
    ['guitar_pro', t('Guitar Pro (.gp)')],
    ['musicxml', t('MusicXML')],
    ['midi', t('MIDI')],
    ['ascii', t('Text tab')],
    ['pdf', t('PDF')],
    ['png', t('Picture (PNG)')],
    ['svg', t('Picture (SVG)')],
  ];

  let files = $state<OpenedFile[]>([]);
  let target = $state<BatchTarget>(stored());
  let results = $state<Converted[]>([]);
  let busy = $state(false);
  let summary = $state('');

  function stored(): BatchTarget {
    try {
      const value = localStorage.getItem(TARGET_KEY);
      if (value && value in TARGET_EXTENSIONS) return value as BatchTarget;
    } catch {
      // The first target is as good a start as any.
    }
    return 'pdf';
  }

  async function add() {
    const picked = await openFiles();
    // A file picked twice is converted once.
    const known = new Set(files.map((f) => f.path ?? f.name));
    files = [...files, ...picked.filter((f) => !known.has(f.path ?? f.name))];
  }

  async function convert() {
    if (!files.length || busy) return;
    try {
      localStorage.setItem(TARGET_KEY, target);
    } catch {
      // Only a convenience.
    }
    busy = true;
    results = [];
    summary = '';
    try {
      summary = (await onconvert($state.snapshot(files) as OpenedFile[], target, (result) => results.push(result))) ?? '';
    } catch (e) {
      summary = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog
  title={t('Batch convert')}
  hint={canPickFolder()
    ? t('Opens each score and writes it in another format, into a folder you choose.')
    : t('Opens each score and writes it in another format. They are downloaded together as a zip file.')}
  width="520px"
  {onclose}
  onsubmit={convert}
>
  <div class="files">
    <span class="heading" id="batch-files">{t('Scores')}</span>
    {#if files.length}
      <ul aria-labelledby="batch-files">
        {#each files as file, i (file.path ?? file.name)}
          <li>
            <span class="name" title={file.path ?? file.name}>{file.name}</span>
            <button
              type="button"
              class="tool"
              aria-label={t('Take {name} out', { name: file.name })}
              disabled={busy}
              onclick={() => files.splice(i, 1)}
            >
              <Icon name="remove" size={14} />
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">{t('None yet: add Guitar Pro, MusicXML, MIDI, text tab or gtrTab files.')}</p>
    {/if}
    <button type="button" class="btn" disabled={busy} onclick={add}>{t('Add scores…')}</button>
  </div>

  <label>
    {t('Convert to')}
    <select bind:value={target} disabled={busy}>
      {#each targets as [value, name]}<option {value}>{name}</option>{/each}
    </select>
  </label>

  {#if results.length || summary}
    <div class="results" role="status" aria-live="polite">
      <ul aria-label={t('Converted')}>
        {#each results as result}
          <li class:failed={result.failed}>
            <span class="name">{result.name ? `${result.from} → ${result.name}` : result.from}</span>
            {#if result.note}<span class="note">{result.failed ? t('Failed: {reason}', { reason: result.note }) : result.note}</span>{/if}
          </li>
        {/each}
      </ul>
      {#if summary}<p>{summary}</p>{/if}
    </div>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>{t('Close')}</button>
    <button type="submit" class="btn primary" disabled={!files.length || busy}>
      {busy ? t('Converting…') : t('Convert')}
    </button>
  {/snippet}
</Dialog>

<style>
  .files {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    margin-bottom: var(--space-5);
  }

  .heading {
    color: var(--muted);
    font-size: var(--text-sm);
  }

  ul {
    align-self: stretch;
    max-height: 200px;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 var(--space-3);
    padding: var(--space-2) var(--space-3);
  }

  li + li {
    border-top: 1px solid var(--border);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note {
    flex-basis: 100%;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .failed .note {
    color: var(--error);
  }

  .empty,
  .results p {
    margin: 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .results {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
</style>
