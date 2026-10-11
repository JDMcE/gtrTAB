<script lang="ts">
  import { ICONS, type IconName } from './icons';

  interface Props {
    name: IconName;
    size?: number;
  }

  let { name, size = 18 }: Props = $props();
  const def = $derived(ICONS[name]);
</script>

{#if 'lucide' in def}
  <def.lucide {size} strokeWidth={1.75} aria-hidden="true" />
{:else}
  <svg class="icon" viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
    {#if 'glyph' in def}
      <text class="glyph" x={def.x} y={def.y} font-size={def.size}>{def.glyph}</text>
    {:else}
      {@html def.svg}
    {/if}
  </svg>
{/if}

<style>
  .icon {
    display: block;
    overflow: visible;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .glyph {
    font-family: var(--font-music);
    text-anchor: middle;
    fill: currentColor;
    stroke: none;
  }
</style>
