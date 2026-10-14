<script lang="ts">
  import type { Found } from './search';
  import { search } from './search';
  import Icon from './ui/Icon.svelte';
  import { shownKeys } from './keys.svelte';

  interface Props {
    /** Every command in the menus. */
    commands: Found[];
    /** Ids of the commands used lately, the latest first. */
    recent: string[];
    /** Closes the palette, then runs what was chosen, if anything. */
    onclose: (chosen?: Found) => void;
  }

  let { commands, recent, onclose }: Props = $props();

  let dialog: HTMLDialogElement;
  let list = $state<HTMLElement>();
  let query = $state('');
  let at = $state(0);

  const found = $derived(search(commands, query, recent));
  const option = (i: number) => `command-${i}`;

  $effect(() => {
    dialog.showModal();
  });

  // Back to the top of the list whenever the query changes.
  $effect(() => {
    void query;
    at = 0;
  });

  // The option moved to by the keyboard stays in view.
  $effect(() => {
    list?.querySelector(`#${option(at)}`)?.scrollIntoView({ block: 'nearest' });
  });

  function choose(i: number) {
    const item = found[i];
    if (!item || item.command.disabled?.()) return;
    onclose(item);
  }

  function onKeydown(event: KeyboardEvent) {
    const last = found.length - 1;
    const page = 8;
    const move = (to: number) => {
      event.preventDefault();
      at = Math.max(0, Math.min(last, to));
    };
    switch (event.key) {
      case 'ArrowDown': return move(at >= last ? 0 : at + 1);
      case 'ArrowUp': return move(at <= 0 ? last : at - 1);
      case 'PageDown': return move(at + page);
      case 'PageUp': return move(at - page);
      case 'Enter':
        event.preventDefault();
        return choose(at);
    }
  }
</script>

<dialog bind:this={dialog} aria-label="Command palette" onclose={() => onclose()}>
  <div class="search">
    <Icon name="search" size={16} />
    <input
      type="text"
      role="combobox"
      aria-label="Command"
      aria-expanded="true"
      aria-controls="commands"
      aria-autocomplete="list"
      aria-activedescendant={found.length ? option(at) : undefined}
      placeholder="Type a command"
      autocomplete="off"
      spellcheck="false"
      bind:value={query}
      onkeydown={onKeydown}
    />
  </div>
  <ul id="commands" role="listbox" aria-label="Commands" bind:this={list}>
    {#each found as item, i (item.id)}
      {@const disabled = item.command.disabled?.() ?? false}
      {@const checked = item.command.active?.()}
      {@const keys = shownKeys(item.command)}
      <!-- The keyboard stays in the field; the list follows it there. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <li
        id={option(i)}
        role="option"
        aria-selected={i === at}
        aria-disabled={disabled}
        aria-checked={checked}
        onpointermove={() => (at = i)}
        onclick={() => choose(i)}
      >
        <span class="mark">
          {#if checked}<Icon name="check" size={14} />{:else if item.command.icon}<Icon name={item.command.icon} size={14} />{/if}
        </span>
        <span class="label">{item.command.label}</span>
        <span class="path">{item.path.join(' › ')}</span>
        {#if keys}<kbd>{keys}</kbd>{/if}
      </li>
    {:else}
      <li class="none" role="presentation">No command matches “{query}”</li>
    {/each}
  </ul>
</dialog>

<style>
  dialog {
    width: min(620px, 94vw);
    max-height: min(480px, 80vh);
    margin-top: 12vh;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--text);
    box-shadow: var(--shadow);
  }

  dialog[open] {
    display: flex;
    flex-direction: column;
  }

  dialog::backdrop {
    background: var(--backdrop);
  }

  .search {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }

  /* The bar around it is the field's frame, and the open palette shows where the focus is. */
  .search input {
    flex: 1;
    height: 44px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font-size: var(--text-lg);
    outline: none;
  }

  ul {
    margin: 0;
    padding: var(--space-2);
    overflow-y: auto;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 30px;
    padding: 0 var(--space-3) 0 var(--space-2);
    border-radius: var(--radius-s);
    cursor: pointer;
  }

  li[aria-selected='true'] {
    background: var(--pressed);
  }

  li[aria-disabled='true'] {
    color: var(--muted);
    opacity: 0.6;
    cursor: default;
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    width: 18px;
    color: var(--muted);
  }

  [aria-checked='true'] .mark {
    color: var(--accent);
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path {
    flex: 1;
    overflow: hidden;
    color: var(--muted);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .none {
    justify-content: center;
    min-height: 60px;
    color: var(--muted);
    cursor: default;
  }
</style>
