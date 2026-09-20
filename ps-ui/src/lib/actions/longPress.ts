export type LongPressOptions = {
	onLongPress: () => void;
	enabled?: boolean;
	delayMs?: number;
	moveTolerancePx?: number;
};

const DEFAULT_DELAY_MS = 500;
const DEFAULT_MOVE_TOLERANCE_PX = 10;

export function longPress(node: HTMLElement, options: LongPressOptions) {
	let current = options;
	let timer: ReturnType<typeof setTimeout> | null = null;
	let startX = 0;
	let startY = 0;
	// A fired long-press still emits a click on release; swallow it.
	let suppressNextClick = false;

	function cancel(): void {
		if (timer !== null) {
			clearTimeout(timer);
			timer = null;
		}
	}

	function handlePointerDown(event: PointerEvent): void {
		if (current.enabled === false) return;
		startX = event.clientX;
		startY = event.clientY;
		cancel();
		timer = setTimeout(() => {
			timer = null;
			if (current.enabled === false) return;
			suppressNextClick = true;
			// Absent in some browsers; iOS Safari ignores it outside a user gesture.
			navigator.vibrate?.(10);
			current.onLongPress();
		}, current.delayMs ?? DEFAULT_DELAY_MS);
	}

	function handlePointerMove(event: PointerEvent): void {
		if (timer === null) return;
		const tolerance = current.moveTolerancePx ?? DEFAULT_MOVE_TOLERANCE_PX;
		if (
			Math.abs(event.clientX - startX) > tolerance ||
			Math.abs(event.clientY - startY) > tolerance
		) {
			cancel();
		}
	}

	function handleClick(event: MouseEvent): void {
		if (!suppressNextClick) return;
		suppressNextClick = false;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	node.addEventListener('pointerdown', handlePointerDown);
	node.addEventListener('pointermove', handlePointerMove);
	node.addEventListener('pointerup', cancel);
	node.addEventListener('pointercancel', cancel);
	node.addEventListener('pointerleave', cancel);
	// Capture phase so the suppressed click never reaches the element's own handler.
	node.addEventListener('click', handleClick, true);

	return {
		update(next: LongPressOptions): void {
			current = next;
		},
		destroy(): void {
			cancel();
			node.removeEventListener('pointerdown', handlePointerDown);
			node.removeEventListener('pointermove', handlePointerMove);
			node.removeEventListener('pointerup', cancel);
			node.removeEventListener('pointercancel', cancel);
			node.removeEventListener('pointerleave', cancel);
			node.removeEventListener('click', handleClick, true);
		}
	};
}
