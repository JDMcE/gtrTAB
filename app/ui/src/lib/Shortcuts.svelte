<script lang="ts">
  import type { ShortcutGroup } from './commands';
  import { bind, chordOf, defaultsOf, fixedKey, keysOf, reset, unbind, type Bindings } from './keymap';
  import { search, type Found } from './search';
  import { t } from './i18n';
  import Dialog from './ui/Dialog.svelte';
  import Icon from './ui/Icon.svelte';
  import { tip } from './ui/tooltip';

  interface Props {
    /** Keys that are not commands, shown as they are. */
    fixed: ShortcutGroup[];
    /** The commands that can be given keys. */
    commands: Found[];
    /** The keys the user has changed. */
    custom: Bindings;
    onchange: (custom: Bindings) => void;
    onclose: () => void;
  }

  let { fixed, commands, custom, onchange, onclose }: Props = $props();

  let query = $state('');
  let every = $state(false);
  /** The command waiting for a key to be pressed. */
  let waiting = $state<string | null>(null);
  let said = $state('');

  const bindable = $derived(commands.map(({ command }) => ({ id: command.id!, keys: command.keys })));
  const byId = $derived(new Map(commands.map((found) => [found.command.id!, found])));
  const name = (id: string) => byId.get(id)?.command.label.replace(/…$/, '') ?? id;
  const keys = (found: Found) => keysOf({ id: found.command.id!, keys: found.command.keys }, custom);
  const changed = (found: Found) => found.command.id! in custom;

  /** The commands shown, under the menus they are in; with nothing typed, only those with keys. */
  const shown = $derived.by(() => {
    const list = query.trim()
      ? search(commands, query)
      : commands.filter((found) => every || keys(found).length || changed(found));
    const menus = new Map<string, Found[]>();
    for (const found of list) menus.set(found.path[0], [...(menus.get(found.path[0]) ?? []), found]);
    return [...menus];
  });

  const fixedShown = $derived(
    query.trim()
      ? fixed
          .map((group) => ({
            ...group,
            rows: group.rows.filter((row) => row.label.toLowerCase().includes(query.trim().toLowerCase())),
          }))
          .filter((group) => group.rows.length)
      : fixed,
  );

  function give(id: string, chord: string) {
    const { bindings, from } = bind(bindable, custom, id, chord);
    onchange(bindings);
    said = from
      ? t('{key} now runs {command}, and no longer {other}.', { key: chord, command: name(id), other: name(from) })
      : t('{key} now runs {command}.', { key: chord, command: name(id) });
  }

  function take(found: Found, chord: string) {
    onchange(unbind(bindable, custom, found.command.id!, chord));
    said = t('{key} no longer runs {command}.', { key: chord, command: name(found.command.id!) });
  }

  function putBack(found: Found) {
    const id = found.command.id!;
    onchange(reset(bindable, custom, id));
    const own = defaultsOf({ id, keys: found.command.keys });
    said = own.length
      ? t('{command} has {keys} again.', { command: name(id), keys: own.join(' / ') })
      : t('{command} has no key again.', { command: name(id) });
  }

  function putAllBack() {
    onchange({});
    said = t('Every command has the keys it came with.');
  }

  /** While a command waits for a key, the next one pressed is given to it. */
  function onKeydown(event: KeyboardEvent) {
    if (!waiting) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      waiting = null;
      said = t('No key given.');
      return;
    }
    const chord = chordOf(event);
    if (!chord) return;
    if (fixedKey(chord)) {
      said = t('{key} moves the cursor, enters notes or moves between controls, and cannot be given to a command. Press another key, or Esc.', { key: chord });
      return;
    }
    const id = waiting;
    waiting = null;
    give(id, chord);
  }
</script>

<svelte:window onkeydowncapture={onKeydown} />

<Dialog
  title={t('Keyboard shortcuts')}
  hint={t('Give a command another key with +, or take one away by clicking it. A key given to one command is taken from any other that had it.')}
  width="900px"
  {onclose}
>
  <div class="find">
    <input type="search" aria-label={t('Find a command')} placeholder={t('Find a command')} bind:value={query} />
    <label class="check">
      <input type="checkbox" bind:checked={every} disabled={!!query.trim()} />
      {t('Every command, not only those with keys')}
    </label>
  </div>
  <p class="said" role="status">{said}</p>

  <div class="groups">
    {#each shown as [menu, items] (menu)}
      <section>
        <h3>{menu}</h3>
        <ul>
          {#each items as found (found.command.id)}
            {@const id = found.command.id!}
            {@const label = found.command.label.replace(/…$/, '')}
            <li>
              <span class="name">
                {label}
                {#if found.path.length > 1}<small>{found.path.slice(1).join(' › ')}</small>{/if}
              </span>
              <span class="keys">
                {#each keys(found) as chord (chord)}
                  <button
                    type="button"
                    class="key"
                    aria-label={t('Take {key} from {command}', { key: chord, command: label })}
                    use:tip={{ label: t('Take this key away') }}
                    onclick={() => take(found, chord)}
                  >
                    <kbd>{chord}</kbd>
                  </button>
                {/each}
                {#if waiting === id}
                  <kbd class="waiting" aria-live="polite">{t('Press a key…')}</kbd>
                {/if}
                <button
                  type="button"
                  class="tool"
                  aria-label={t('Give {command} a key', { command: label })}
                  aria-pressed={waiting === id}
                  use:tip={{ label: t('Give it a key') }}
                  onclick={() => {
                    waiting = waiting === id ? null : id;
                    said = waiting ? t('Press the key for {command}, or Esc.', { command: label }) : '';
                  }}
                >
                  <Icon name="addSound" size={14} />
                </button>
                {#if changed(found)}
                  <button
                    type="button"
                    class="tool"
                    aria-label={t('Give {command} its own keys back', { command: label })}
                    use:tip={{ label: t('Back to its own keys') }}
                    onclick={() => putBack(found)}
                  >
                    <Icon name="undo" size={14} />
                  </button>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/each}

    {#each fixedShown as group (group.title)}
      <section class="fixed">
        <h3>{group.title}</h3>
        <ul>
          {#each group.rows as row}
            <li>
              <span class="name">{row.label}</span>
              <span class="keys">
                {#each row.keys.split(' / ') as keys, i}
                  {#if i > 0}<span class="or">{t('or')}</span>{/if}
                  <kbd>{keys}</kbd>
                {/each}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/each}

    {#if !shown.length && !fixedShown.length}
      <p class="none">{t('No command matches “{query}”', { query })}</p>
    {/if}
  </div>

  {#snippet footer()}
    <button type="button" class="btn remove" disabled={!Object.keys(custom).length} onclick={putAllBack}>
      {t('Reset every key')}
    </button>
    <button type="button" class="btn primary" onclick={onclose}>{t('Done')}</button>
  {/snippet}
</Dialog>

<style>
  .find {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-5);
  }

  .find input[type='search'] {
    flex: 1 1 240px;
  }

  .find .check {
    margin: 0;
  }

  .said {
    min-height: 1.4em;
    margin: var(--space-3) 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .groups {
    max-height: 58vh;
    overflow-y: auto;
    columns: 2 340px;
    column-gap: var(--space-6);
  }

  section {
    break-inside: avoid;
    margin-bottom: var(--space-5);
  }

  h3 {
    margin: 0 0 var(--space-2);
    color: var(--accent);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .fixed h3 {
    color: var(--muted);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: 28px;
    border-bottom: 1px solid var(--border);
    font-size: var(--text-sm);
  }

  .name small {
    display: block;
    color: var(--muted);
    font-size: var(--text-xs);
  }

  .keys {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  .key {
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }

  .key:hover kbd {
    border-color: var(--error);
    color: var(--error);
    text-decoration: line-through;
  }

  .waiting {
    border-style: dashed;
    color: var(--accent);
  }

  .tool {
    width: 24px;
    height: 24px;
  }

  .or {
    color: var(--muted);
    font-size: var(--text-xs);
  }

  .none {
    color: var(--muted);
  }
</style>
