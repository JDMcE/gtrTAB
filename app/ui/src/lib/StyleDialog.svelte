<script lang="ts">
  import { untrack } from 'svelte';
  import { t } from './i18n';
  import type { BarNumbers, Style, TabRhythm } from './model';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** The score's stylesheet now. */
    style: Style;
    /** The stylesheet a score comes with. */
    standard: Style;
    onapply: (style: Style) => void;
    onclose: () => void;
  }

  let { style, standard, onapply, onclose }: Props = $props();

  // The fields start from the score's stylesheet; `null` is a field left empty.
  let fields = $state<{ [K in keyof Style]: Style[K] | null }>(untrack(() => ({ ...style })));

  const clamp = (value: number | null, [low, high]: [number, number], fallback: number) =>
    Math.max(low, Math.min(high, Math.round(value ?? fallback)));

  const SIZES: [number, number] = [50, 200];
  const SPACINGS: [number, number] = [50, 300];
  const GAPS: [number, number] = [0, 120];
  const TEXT: [number, number] = [6, 72];

  const numbers: [BarNumbers, string][] = [
    ['every', t('Over every bar')],
    ['lines', t('Over the first bar of each line')],
    ['none', t('Not written')],
  ];
  const rhythms: [TabRhythm, string][] = [
    ['alone', t('Only without notation over it')],
    ['always', t('Always')],
    ['never', t('Never')],
  ];

  function submit() {
    onapply({
      size: clamp(fields.size, SIZES, standard.size),
      spacing: clamp(fields.spacing, SPACINGS, standard.spacing),
      line_gap: clamp(fields.line_gap, GAPS, standard.line_gap),
      track_gap: clamp(fields.track_gap, GAPS, standard.track_gap),
      staff_gap: clamp(fields.staff_gap, GAPS, standard.staff_gap),
      title_size: clamp(fields.title_size, TEXT, standard.title_size),
      subtitle_size: clamp(fields.subtitle_size, TEXT, standard.subtitle_size),
      bar_numbers: fields.bar_numbers ?? standard.bar_numbers,
      tab_rhythm: fields.tab_rhythm ?? standard.tab_rhythm,
      tempo: fields.tempo ?? standard.tempo,
      tuning: fields.tuning ?? standard.tuning,
      bars_per_line: fields.bars_per_line ? clamp(fields.bars_per_line, [1, 32], 4) : null,
    });
    onclose();
  }
</script>

<Dialog
  title={t('Stylesheet')}
  hint={t('How the score is set out, on the screen and on paper. It is kept with the score.')}
  width="560px"
  {onclose}
  onsubmit={submit}
>
  <div class="sections">
    <section>
      <h3>{t('Size and spacing')}</h3>
      <label>{t('Size of the music (%)')} <input type="number" min={SIZES[0]} max={SIZES[1]} bind:value={fields.size} /></label>
      <label>{t('Note spacing (%)')} <input type="number" min={SPACINGS[0]} max={SPACINGS[1]} bind:value={fields.spacing} /></label>
      <label>{t('Between lines (px)')} <input type="number" min={GAPS[0]} max={GAPS[1]} bind:value={fields.line_gap} /></label>
      <label>{t('Between tracks (px)')} <input type="number" min={GAPS[0]} max={GAPS[1]} bind:value={fields.track_gap} /></label>
      <label>{t('Between notation and tab (px)')} <input type="number" min={GAPS[0]} max={GAPS[1]} bind:value={fields.staff_gap} /></label>
      <label>
        {t('Bars on each line (empty for as many as fit)')}
        <input type="number" min="1" max="32" bind:value={fields.bars_per_line} />
      </label>
    </section>
    <section>
      <h3>{t('Heading')}</h3>
      <label>{t('Title (pt)')} <input type="number" min={TEXT[0]} max={TEXT[1]} bind:value={fields.title_size} /></label>
      <label>{t('Subtitle (pt)')} <input type="number" min={TEXT[0]} max={TEXT[1]} bind:value={fields.subtitle_size} /></label>
      <label class="check"><input type="checkbox" bind:checked={fields.tuning} /> {t('Tunings and capos not as usual')}</label>
      <label class="check"><input type="checkbox" bind:checked={fields.tempo} /> {t('Tempo over the first bar')}</label>
      <h3>{t('Conventions')}</h3>
      <label>
        {t('Bar numbers')}
        <select bind:value={fields.bar_numbers}>
          {#each numbers as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
      <label>
        {t('Rhythm under the tab')}
        <select bind:value={fields.tab_rhythm}>
          {#each rhythms as [value, name]}<option {value}>{name}</option>{/each}
        </select>
      </label>
    </section>
  </div>
  {#snippet footer()}
    <button type="button" class="btn remove" onclick={() => (fields = { ...standard })}>{t('As it comes')}</button>
    <button type="button" class="btn" onclick={onclose}>{t('Cancel')}</button>
    <button type="submit" class="btn primary">{t('OK')}</button>
  {/snippet}
</Dialog>

<style>
  .sections {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0 var(--space-6);
  }

  select {
    min-width: 0;
    max-width: 100%;
  }

  h3 {
    margin: 0 0 var(--space-3);
    color: var(--accent);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
</style>
