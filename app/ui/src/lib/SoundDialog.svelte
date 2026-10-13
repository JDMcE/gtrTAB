<script lang="ts">
  // Names one of a track's other sounds and says which instrument it is the sound of.
  import { untrack } from 'svelte';
  import { INSTRUMENTS, type Instrument, type Sound } from './model';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** The sound to change, or `null` for a new one. */
    sound: Sound | null;
    /** The track's own instrument: a drum track's sounds are drums, and only its are. */
    instrument: Instrument;
    onapply: (name: string, instrument: Instrument) => void;
    onclose: () => void;
  }

  let { sound, instrument, onapply, onclose }: Props = $props();

  const drums = untrack(() => instrument === 'drums');
  const choices = INSTRUMENTS.filter(([kind]) => (kind === 'drums') === drums);
  let name = $state(untrack(() => sound?.name ?? ''));
  let chosen = $state<Instrument>(untrack(() => sound?.instrument ?? (drums ? 'drums' : 'distortion_guitar')));

  function submit() {
    if (!name.trim()) return;
    onapply(name.trim(), chosen);
    onclose();
  }
</script>

<Dialog
  title={sound ? 'Sound' : 'Add a sound'}
  hint="A sound the track can switch to from any beat, with Beat > Change of sound: the sound of an instrument, through pedals of its own, set on the pedalboard. What is written stays as it is."
  {onclose}
  onsubmit={submit}
>
  <label>Name <input bind:value={name} maxlength="40" required /></label>
  <label>
    The sound of
    <select bind:value={chosen}>
      {#each choices as [kind, label]}<option value={kind}>{label}</option>{/each}
    </select>
  </label>
  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">OK</button>
  {/snippet}
</Dialog>
