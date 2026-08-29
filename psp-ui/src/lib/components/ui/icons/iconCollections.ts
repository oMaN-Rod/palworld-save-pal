/**
 * Registers the app's Iconify icon collections with the offline storage
 * backing `<Icon>` (`./Icon.svelte`). Everything is bundled locally — no icon
 * data is ever fetched from the Iconify API at runtime, which keeps the
 * desktop and webapp builds fully offline-capable.
 *
 * The design language is Tabler (stroke-based), with Phosphor fills for gaps,
 * `svg-spinners` for loading states and `line-md` for animated flourishes.
 * Only the icons actually used in `src/` are bundled — regenerate the subset
 * file after adding or removing icons:
 *
 *   node scripts/extract-icon-subsets.mjs
 */
import { registerIconSubsets } from './subsets.generated';

registerIconSubsets();
