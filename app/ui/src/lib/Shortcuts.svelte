<script lang="ts">
  import type { ShortcutGroup } from './commands';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    groups: ShortcutGroup[];
    onclose: () => void;
  }

  let { groups, onclose }: Props = $props();
</script>

<Dialog title="Keyboard shortcuts" width="860px" {onclose}>
  <div class="groups">
    {#each groups as group}
      <section>
        <h3>{group.title}</h3>
        <dl>
          {#each group.rows as row}
            <div>
              <dt>{row.label}</dt>
              <dd>
                {#each row.keys.split(' / ') as keys, i}
                  {#if i > 0}<span>or</span>{/if}
                  <kbd>{keys}</kbd>
                {/each}
              </dd>
            </div>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
</Dialog>

<style>
  .groups {
    columns: 3 230px;
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

  dl {
    margin: 0;
  }

  dl div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: 24px;
    border-bottom: 1px solid var(--border);
    font-size: var(--text-sm);
  }

  dd {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
  }

  dd span {
    color: var(--muted);
    font-size: var(--text-xs);
  }
</style>
