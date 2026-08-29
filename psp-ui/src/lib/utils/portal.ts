import type { Attachment } from 'svelte/attachments';

export function portal(target: HTMLElement | string = 'body'): Attachment {
	return (node) => {
		const el = typeof target === 'string' ? document.querySelector(target) : target;
		el?.appendChild(node as Node);
		return () => {
			(node as ChildNode).remove();
		};
	};
}
