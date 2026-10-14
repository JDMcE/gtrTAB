// The keys the user has given commands, shared by everything that shows a command's key.

import { keysOf, type Bindings } from './keymap';
import type { Command } from './commands';

export const shortcuts = $state<{ custom: Bindings }>({ custom: {} });

/** The key to show beside a command: the first it has now. */
export function shownKeys(command: Command): string | undefined {
  return (command.id ? keysOf({ id: command.id, keys: command.keys }, shortcuts.custom) : command.keys?.split(' / '))?.[0];
}
