<script lang="ts">
  import { t } from './i18n';
  import type { PaletteSection } from './commands';
  import Icon from './ui/Icon.svelte';
  import { roving } from './ui/roving';
  import ToolButton from './ui/ToolButton.svelte';
  import { tip } from './ui/tooltip';

  interface Props {
    sections: PaletteSection[];
    /** A two-column strip with the section titles reduced to their arrows. */
    compact: boolean;
    /** Ids of the folded sections. */
    collapsed: string[];
    ontoggle: (id: string) => void;
  }

  let { sections, compact, collapsed, ontoggle }: Props = $props();
</script>

<aside class="palette" class:compact aria-label={t('Tools')} use:roving>
  {#each sections as section}
    {@const folded = collapsed.includes(section.id)}
    <section>
      <button
        type="button"
        class="title"
        aria-expanded={!folded}
        aria-label={section.title}
        use:tip={{ label: folded ? t('Show {section}', { section: section.title }) : t('Hide {section}', { section: section.title }), side: 'right' }}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => ontoggle(section.id)}
      >
        <Icon name={folded ? 'chevronRight' : 'chevronDown'} size={13} />
        <span>{section.title}</span>
      </button>
      {#if !folded}
        <div class="tools" role="group" aria-label={section.title}>
          {#each section.commands as command}
            <ToolButton {command} side="right" />
          {/each}
        </div>
      {/if}
    </section>
  {/each}
</aside>

<style>
  .palette {
    flex: none;
    /* Four tools across; the panel is as wide as that plus its scrollbar. */
    --columns: 4;
    padding: var(--space-2) var(--space-2) var(--space-4) var(--space-3);
    overflow-y: auto;
    scrollbar-width: thin;
    /* Keeps the columns the same whether or not the tools need to scroll. */
    scrollbar-gutter: stable;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }

  .palette.compact {
    --columns: 2;
  }

  section {
    width: calc(var(--columns) * (var(--control) + var(--space-1)) - var(--space-1));
  }

  .title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
    cursor: pointer;
  }

  .title:hover {
    color: var(--text);
  }

  .compact .title span {
    flex: 1;
    height: 1px;
    overflow: hidden;
    background: var(--border);
    color: transparent;
  }

  .tools {
    display: grid;
    grid-template-columns: repeat(var(--columns), var(--control));
    gap: var(--space-1);
    margin-bottom: var(--space-2);
  }
</style>
