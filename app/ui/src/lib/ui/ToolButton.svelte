<script lang="ts">
  import type { Command } from '../commands';
  import Icon from './Icon.svelte';
  import { tip } from './tooltip';
  import { aria, keyOf, shownKeys } from '../keys.svelte';

  interface Props {
    command: Command;
    side?: 'bottom' | 'right';
    size?: number;
  }

  let { command, side = 'bottom', size = 20 }: Props = $props();
  const disabled = $derived(command.disabled?.() ?? false);
  const keys = $derived(keyOf(command));
</script>

<!-- Disabled tools stay focusable so that their tooltip still says what they are. A click
     must not take the focus away from the score, hence the mousedown handler. -->
<button
  type="button"
  class="tool"
  aria-label={command.label}
  aria-keyshortcuts={aria(shownKeys(command))}
  aria-pressed={command.active ? command.active() : undefined}
  aria-disabled={disabled}
  use:tip={{ label: command.label, keys, side }}
  onmousedown={(e) => e.preventDefault()}
  onclick={() => !disabled && command.run()}
>
  {#if command.icon}<Icon name={command.icon} {size} />{/if}
</button>
