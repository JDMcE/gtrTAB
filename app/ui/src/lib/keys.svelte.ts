// The keys the user has given commands, shared by everything that shows a command's key.

import { ariaKeys, keyLabel, keysOf, type Bindings } from './keymap';
import type { Command } from './commands';

export const shortcuts = $state<{ custom: Bindings }>({ custom: {} });

/** Whether the app runs on a Mac, whose keys are shown with its own symbols. */
export const MAC = /mac|iphone|ipad/i.test(
  (navigator as Navigator & { userAgentData?: { platform: string } }).userAgentData?.platform ?? navigator.platform ?? '',
);

/** The key a command has now, the first of them if it has several. */
export function shownKeys(command: Command): string | undefined {
  return (command.id ? keysOf({ id: command.id, keys: command.keys }, shortcuts.custom) : command.keys?.split(' / '))?.[0];
}

/** Keys as they are shown on this system. */
export const label = (keys: string) => keyLabel(keys, MAC);

/** Keys as a screen reader is given them, in `aria-keyshortcuts`. */
export const aria = (keys: string | undefined) => (keys ? ariaKeys(keys, MAC) : undefined);

/** The key shown beside a command, as this system shows it. */
export function keyOf(command: Command): string | undefined {
  const keys = shownKeys(command);
  return keys && label(keys);
}
