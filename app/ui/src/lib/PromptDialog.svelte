<script lang="ts" module>
  /** One thing a prompt asks for. */
  export type PromptField =
    | { kind: 'text'; label: string; value: string }
    /** One entry per line, such as a syllable for each line of lyrics. */
    | { kind: 'lines'; label: string; value: string[] }
    /** A whole number; left empty it is `null`. */
    | { kind: 'number'; label: string; value: number | null; min: number; max: number }
    /** A choice of any of the numbers from 1 to `count`, as one bit each. */
    | { kind: 'bits'; label: string; value: number; count: number };

  export interface Prompt {
    title: string;
    hint?: string;
    field: PromptField;
    /** Receives the field's new value, in the type its `value` has. */
    apply: (value: never) => void;
  }
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    prompt: Prompt;
    onclose: () => void;
  }

  let { prompt, onclose }: Props = $props();

  const field = untrack(() => prompt.field);
  let text = $state(
    field.kind === 'text' ? field.value
    : field.kind === 'lines' ? field.value.join('\n')
    : '',
  );
  let number = $state(field.kind === 'number' ? field.value : null);
  let bits = $state(field.kind === 'bits' ? field.value : 0);

  function submit() {
    const apply = prompt.apply as (value: unknown) => void;
    if (field.kind === 'text') apply(text.trim());
    else if (field.kind === 'lines') {
      const lines = text.split('\n').map((line) => line.trim());
      while (lines.length && !lines[lines.length - 1]) lines.pop();
      apply(lines);
    } else if (field.kind === 'number') apply(number);
    else apply(bits);
    onclose();
  }
</script>

<Dialog title={prompt.title} hint={prompt.hint} {onclose} onsubmit={submit}>
  {#if field.kind === 'text'}
    <label>{field.label} <input bind:value={text} maxlength="120" /></label>
  {:else if field.kind === 'lines'}
    <label>
      {field.label}
      <textarea rows="4" bind:value={text}></textarea>
    </label>
  {:else if field.kind === 'number'}
    <label>
      {field.label}
      <input type="number" min={field.min} max={field.max} bind:value={number} />
    </label>
  {:else}
    <fieldset>
      <legend>{field.label}</legend>
      {#each { length: field.count } as _, i}
        <label class="check">
          <input
            type="checkbox"
            checked={(bits & (1 << i)) !== 0}
            onchange={(e) => (bits = e.currentTarget.checked ? bits | (1 << i) : bits & ~(1 << i))}
          />
          {i + 1}
        </label>
      {/each}
    </fieldset>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">OK</button>
  {/snippet}
</Dialog>

<style>
  textarea {
    padding: var(--space-3);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--raised);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }

  fieldset {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: var(--space-3);
    padding: 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  fieldset :global(label.check) {
    margin-bottom: 0;
  }
</style>
