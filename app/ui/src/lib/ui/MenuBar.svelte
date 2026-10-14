<script lang="ts">
  import { t } from '../i18n';
  import { tick } from 'svelte';
  import type { Command, Menu, MenuEntry } from '../commands';
  import Icon from './Icon.svelte';
  import { shownKeys } from '../keys.svelte';

  interface Props {
    menus: Menu[];
    /** Called when a menu closes without the focus staying in the bar. */
    onclose: () => void;
  }

  let { menus, onclose }: Props = $props();

  let bar: HTMLElement;
  /** Index of the open menu. */
  let open = $state<number | null>(null);
  /** Label of the open submenu. */
  let sub = $state<string | null>(null);

  const tops = () => [...bar.querySelectorAll<HTMLElement>(':scope > .slot > button')];
  const itemsOf = (menu: Element) =>
    [...menu.querySelectorAll<HTMLElement>(':scope > button, :scope > .slot > button')];

  function close() {
    open = null;
    sub = null;
  }

  async function show(index: number, focusFirst = false) {
    open = (index + menus.length) % menus.length;
    sub = null;
    await tick();
    const top = tops()[open];
    if (focusFirst) itemsOf(top.nextElementSibling!)[0]?.focus();
    else top.focus();
  }

  async function showSub(label: string, button: HTMLElement) {
    sub = label;
    await tick();
    itemsOf(button.nextElementSibling!)[0]?.focus();
  }

  function choose(command: Command) {
    if (command.disabled?.()) return;
    close();
    onclose();
    command.run();
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    const menu = target.closest('[role="menu"]');
    const handled = () => {
      event.preventDefault();
      event.stopPropagation();
    };

    if (!menu) {
      const at = tops().indexOf(target);
      switch (event.key) {
        case 'ArrowRight': handled(); open === null ? tops()[(at + 1) % menus.length].focus() : show(at + 1); break;
        case 'ArrowLeft': handled(); open === null ? tops()[(at - 1 + menus.length) % menus.length].focus() : show(at - 1); break;
        case 'ArrowDown':
        case 'Enter':
        case ' ': handled(); show(at, true); break;
        case 'Escape': handled(); close(); onclose(); break;
      }
      return;
    }

    const items = itemsOf(menu);
    const at = items.indexOf(target);
    const inSub = menu.classList.contains('sub');
    switch (event.key) {
      case 'ArrowDown': handled(); items[(at + 1) % items.length].focus(); break;
      case 'ArrowUp': handled(); items[(at - 1 + items.length) % items.length].focus(); break;
      case 'Home': handled(); items[0].focus(); break;
      case 'End': handled(); items[items.length - 1].focus(); break;
      case 'ArrowRight':
        handled();
        if (target.getAttribute('aria-haspopup')) target.click();
        else show(open! + 1, true);
        break;
      case 'ArrowLeft':
      case 'Escape':
        handled();
        if (inSub) {
          sub = null;
          (menu.previousElementSibling as HTMLElement).focus();
        } else if (event.key === 'Escape') {
          const top = tops()[open!];
          close();
          top.focus();
        } else show(open! - 1, true);
        break;
      case 'Tab': close(); break;
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (open !== null && !bar.contains(event.target as Node)) close();
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} onblur={close} />

{#snippet list(items: MenuEntry[], nested: boolean)}
  <div class="menu" class:sub={nested} role="menu">
    {#each items as entry}
      {#if entry === '-'}
        <div class="rule" role="separator"></div>
      {:else if 'items' in entry}
        <div class="slot">
          <button
            type="button"
            role="menuitem"
            tabindex="-1"
            aria-haspopup="menu"
            aria-expanded={sub === entry.label}
            onpointerenter={() => (sub = entry.label)}
            onclick={(e) => showSub(entry.label, e.currentTarget)}
          >
            <span class="mark"></span>
            <span class="label">{entry.label}</span>
            <Icon name="chevronRight" size={14} />
          </button>
          {#if sub === entry.label}{@render list(entry.items, true)}{/if}
        </div>
      {:else}
        <button
          type="button"
          role={entry.active ? 'menuitemcheckbox' : 'menuitem'}
          tabindex="-1"
          aria-checked={entry.active ? entry.active() : undefined}
          aria-disabled={entry.disabled?.() ?? false}
          aria-keyshortcuts={shownKeys(entry)}
          onpointerenter={() => !nested && (sub = null)}
          onclick={() => choose(entry)}
        >
          <span class="mark">
            {#if entry.active?.()}
              <Icon name="check" size={14} />
            {:else if entry.icon}
              <Icon name={entry.icon} size={15} />
            {/if}
          </span>
          <span class="label">{entry.label}</span>
          {#if shownKeys(entry)}<kbd>{shownKeys(entry)}</kbd>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<nav class="menubar" role="menubar" aria-label={t('Menus')} bind:this={bar} onkeydown={onKeydown}>
  {#each menus as menu, i}
    <div class="slot">
      <button
        type="button"
        class="top"
        role="menuitem"
        tabindex={i === (open ?? 0) ? 0 : -1}
        aria-haspopup="menu"
        aria-expanded={open === i}
        onclick={() => (open === i ? close() : show(i))}
        onpointerenter={() => open !== null && open !== i && show(i)}
      >
        {menu.label}
      </button>
      {#if open === i}{@render list(menu.items, false)}{/if}
    </div>
  {/each}
</nav>

<style>
  /* In a narrow window the menus close up, and wrap rather than run off the side when the
     font is wider than expected. */
  .menubar {
    display: flex;
    flex-wrap: wrap;
    min-width: 0;
  }

  .slot {
    position: relative;
  }

  button {
    border: 0;
    background: none;
    cursor: pointer;
  }

  .top {
    height: var(--control);
    padding: 0 var(--space-3);
    border-radius: var(--radius-s);
  }

  @media (max-width: 520px) {
    .top {
      padding: 0 var(--space-2);
    }
  }

  .top:hover,
  .top[aria-expanded='true'] {
    background: var(--hover);
  }

  .menu {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 50;
    min-width: 220px;
    max-height: calc(100vh - 60px);
    padding: var(--space-2);
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    background: var(--raised);
    box-shadow: var(--shadow);
  }

  /* A submenu must not be clipped by its parent's scrolling. */
  .menu:has(.sub) {
    overflow: visible;
  }

  .menu.sub {
    top: calc(-1 * var(--space-2) - 1px);
    left: 100%;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: 28px;
    padding: 0 var(--space-3) 0 var(--space-2);
    border-radius: var(--radius-s);
    text-align: left;
    white-space: nowrap;
  }

  .menu button:hover,
  .menu button:focus-visible,
  .menu button[aria-expanded='true'] {
    background: var(--hover);
  }

  .menu button[aria-disabled='true'] {
    color: var(--muted);
    opacity: 0.6;
    cursor: default;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--muted);
  }

  [aria-checked='true'] .mark {
    color: var(--accent);
  }

  .label {
    flex: 1;
  }

  .rule {
    height: 1px;
    margin: var(--space-2) 0;
    background: var(--border);
  }
</style>
