import type { Attachment } from 'svelte/attachments';

/**
 * Attachment that moves the element to `document.body` (or a given target) so it
 * escapes any ancestor stacking context or `overflow` clipping.
 *
 * Fixed-positioned floating UI (tooltips, menus, popovers) breaks when an
 * ancestor establishes a stacking context (e.g. `backdrop-filter`, `transform`,
 * `filter`, `opacity`) — its `z-index` then only ranks within that ancestor, so
 * elements elsewhere on the page paint over it. Portaling to the body lifts it
 * back to the page root where its `z-index` applies globally.
 *
 * Usage: `<div {@attach portal()}> … </div>`
 */
export function portal(target: HTMLElement | string = 'body'): Attachment {
	return (node) => {
		const el = typeof target === 'string' ? document.querySelector(target) : target;
		el?.appendChild(node as Node);
		return () => {
			(node as ChildNode).remove();
		};
	};
}
