// Keyboard navigation inside a group of controls: the group is a single Tab stop and the
// arrow keys move between its controls. A slider keeps the up and down arrows, Page Up and
// Down, Home and End for its value, so only the left and right arrows move on from one.

const CONTROLS = 'button, select, input[type="range"]';
const SLIDER_KEYS = ['ArrowUp', 'ArrowDown', 'Home', 'End'];

/** Svelte action for a toolbar or palette. Marks the node so the score's keys leave it alone. */
export function roving(node: HTMLElement) {
  node.dataset.roving = '';

  const controls = () =>
    [...node.querySelectorAll<HTMLElement>(CONTROLS)].filter((c) => c.offsetParent !== null);

  /** Leaves exactly one control in the Tab order. */
  function sync(current?: HTMLElement) {
    const all = controls();
    const keep = current ?? all.find((c) => c.tabIndex === 0) ?? all[0];
    for (const control of all) control.tabIndex = control === keep ? 0 : -1;
  }

  /** The closest control in the row above or below. */
  function vertical(from: HTMLElement, all: HTMLElement[], down: boolean) {
    const a = from.getBoundingClientRect();
    let best: HTMLElement | undefined;
    let bestScore = Infinity;
    for (const control of all) {
      const b = control.getBoundingClientRect();
      const dy = down ? b.top - a.top : a.top - b.top;
      if (dy < a.height / 2) continue;
      const score = dy * 4 + Math.abs(b.left + b.width / 2 - (a.left + a.width / 2));
      if (score < bestScore) [best, bestScore] = [control, score];
    }
    return best;
  }

  function onKeydown(event: KeyboardEvent) {
    const from = event.target as HTMLElement;
    if (from instanceof HTMLSelectElement || event.ctrlKey || event.metaKey || event.altKey) return;
    if (from instanceof HTMLInputElement && SLIDER_KEYS.includes(event.key)) return;
    const all = controls();
    const at = all.indexOf(from);
    if (at < 0) return;
    let to: HTMLElement | undefined;
    switch (event.key) {
      case 'ArrowRight': to = all[(at + 1) % all.length]; break;
      case 'ArrowLeft': to = all[(at - 1 + all.length) % all.length]; break;
      case 'ArrowDown': to = vertical(from, all, true); break;
      case 'ArrowUp': to = vertical(from, all, false); break;
      case 'Home': to = all[0]; break;
      case 'End': to = all[all.length - 1]; break;
      default: return;
    }
    event.preventDefault();
    to?.focus();
  }

  const onFocusin = (event: FocusEvent) => sync(event.target as HTMLElement);
  const observer = new MutationObserver(() => sync());
  observer.observe(node, { childList: true, subtree: true });
  node.addEventListener('keydown', onKeydown);
  node.addEventListener('focusin', onFocusin);
  sync();

  return {
    destroy() {
      observer.disconnect();
      node.removeEventListener('keydown', onKeydown);
      node.removeEventListener('focusin', onFocusin);
    },
  };
}
