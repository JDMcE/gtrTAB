<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    title: string;
    /** Explains the dialog under its title. */
    hint?: string;
    width?: string;
    onclose: () => void;
    onsubmit?: () => void;
    children: Snippet;
    /** Buttons at the bottom; left out for dialogs that only show something. */
    footer?: Snippet;
  }

  let { title, hint, width = '360px', onclose, onsubmit, children, footer }: Props = $props();

  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
    // Start on the first field, not on the close button in the title bar.
    dialog.querySelector<HTMLElement>('.body :is(input, select, button), footer .primary')?.focus();
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    onsubmit?.();
  }
</script>

<dialog bind:this={dialog} style:width="min({width}, 94vw)" aria-label={title} {onclose}>
  <form onsubmit={submit}>
    <header>
      <h2>{title}</h2>
      <button type="button" class="tool" aria-label="Close" onclick={onclose}>
        <Icon name="remove" size={16} />
      </button>
    </header>
    <div class="body">
      {#if hint}<p class="hint">{hint}</p>{/if}
      {@render children()}
    </div>
    {#if footer}
      <footer>{@render footer()}</footer>
    {/if}
  </form>
</dialog>

<style>
  dialog {
    max-height: 92vh;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-l);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow);
  }

  dialog::backdrop {
    background: var(--backdrop);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-5);
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .body {
    padding: var(--space-5);
  }

  .hint {
    margin: 0 0 var(--space-4);
    color: var(--muted);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-top: 1px solid var(--border);
    background: var(--bg);
    border-radius: 0 0 var(--radius-l) var(--radius-l);
  }

  /* Fields inside any dialog: a small label above each control. */
  .body :global(label) {
    display: grid;
    gap: var(--space-2);
    margin-bottom: var(--space-4);
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .body :global(label.check) {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    color: var(--text);
    font-size: var(--text-md);
  }

  .body :global(.problem) {
    margin: 0;
    color: var(--error);
  }

  footer :global(.remove) {
    margin-right: auto;
  }
</style>
