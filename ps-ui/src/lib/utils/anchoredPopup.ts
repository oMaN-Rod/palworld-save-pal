import { autoUpdate, computePosition, flip, offset, shift, size } from '@floating-ui/dom';
import type { Attachment } from 'svelte/attachments';

/** Pins a portaled popup to its trigger so a scrolling ancestor cannot clip it. */
export function anchoredPopup(trigger: HTMLElement | undefined, maxHeight = 320): Attachment {
	return (node) => {
		const popup = node as HTMLElement;
		if (!trigger) return;
		Object.assign(popup.style, { position: 'fixed', left: '0', top: '0' });

		const update = () =>
			computePosition(trigger, popup, {
				placement: 'bottom-start',
				strategy: 'fixed',
				middleware: [
					offset(4),
					flip({ padding: 8 }),
					shift({ padding: 8 }),
					size({
						padding: 8,
						apply({ availableHeight, rects }) {
							Object.assign(popup.style, {
								width: `${rects.reference.width}px`,
								maxHeight: `${Math.min(maxHeight, availableHeight)}px`
							});
						}
					})
				]
			}).then(({ x, y }) => {
				Object.assign(popup.style, { left: `${x}px`, top: `${y}px` });
			});

		return autoUpdate(trigger, popup, update);
	};
}
