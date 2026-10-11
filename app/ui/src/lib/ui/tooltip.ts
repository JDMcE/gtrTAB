// Tooltips giving a control's name and shortcut, shown on hover and on keyboard focus.

export interface Tip {
  label: string;
  keys?: string;
  /** Where the tooltip sits relative to the control. */
  side?: 'bottom' | 'right';
}

const DELAY_MS = 450;
const GAP = 6;

let bubble: HTMLDivElement | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;
let owner: HTMLElement | undefined;

function hide() {
  clearTimeout(timer);
  owner = undefined;
  bubble?.remove();
}

function show(node: HTMLElement, tip: Tip) {
  bubble ??= Object.assign(document.createElement('div'), { className: 'tooltip' });
  bubble.setAttribute('role', 'tooltip');
  bubble.replaceChildren(tip.label);
  if (tip.keys) bubble.append(Object.assign(document.createElement('kbd'), { textContent: tip.keys }));
  document.body.append(bubble);
  owner = node;

  const at = node.getBoundingClientRect();
  const size = bubble.getBoundingClientRect();
  let left = at.left + at.width / 2 - size.width / 2;
  let top = at.bottom + GAP;
  if (tip.side === 'right' && at.right + GAP + size.width <= window.innerWidth) {
    left = at.right + GAP;
    top = at.top + at.height / 2 - size.height / 2;
  } else if (top + size.height > window.innerHeight) {
    top = at.top - GAP - size.height;
  }
  left = Math.max(GAP, Math.min(left, window.innerWidth - size.width - GAP));
  top = Math.max(GAP, Math.min(top, window.innerHeight - size.height - GAP));
  bubble.style.left = `${left}px`;
  bubble.style.top = `${top}px`;
}

/** Svelte action: `use:tip={{ label, keys }}`. */
export function tip(node: HTMLElement, content: Tip) {
  let current = content;
  const enter = () => {
    clearTimeout(timer);
    timer = setTimeout(() => show(node, current), DELAY_MS);
  };
  const focus = () => {
    if (node.matches(':focus-visible')) show(node, current);
  };
  const leave = () => {
    if (owner === node || timer !== undefined) hide();
  };
  const key = (event: KeyboardEvent) => {
    if (event.key === 'Escape') leave();
  };

  node.addEventListener('pointerenter', enter);
  node.addEventListener('pointerleave', leave);
  node.addEventListener('pointerdown', leave);
  node.addEventListener('focus', focus);
  node.addEventListener('blur', leave);
  node.addEventListener('keydown', key);

  return {
    update(next: Tip) {
      current = next;
      if (owner === node) show(node, current);
    },
    destroy() {
      if (owner === node) hide();
      node.removeEventListener('pointerenter', enter);
      node.removeEventListener('pointerleave', leave);
      node.removeEventListener('pointerdown', leave);
      node.removeEventListener('focus', focus);
      node.removeEventListener('blur', leave);
      node.removeEventListener('keydown', key);
    },
  };
}
