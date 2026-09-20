const FOCUSABLE_SELECTOR = [
	'a[href]',
	'button:not([disabled])',
	'textarea:not([disabled])',
	'input:not([disabled])',
	'select:not([disabled])',
	'[tabindex]:not([tabindex="-1"])'
].join(', ');

function focusableElements(container: HTMLElement): HTMLElement[] {
	return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
}

export function trapFocus(container: HTMLElement): () => void {
	const previouslyFocused = document.activeElement as HTMLElement | null;

	const hadTabIndex = container.hasAttribute('tabindex');
	if (!hadTabIndex) container.setAttribute('tabindex', '-1');

	(focusableElements(container)[0] ?? container).focus();

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Tab') return;

		const elements = focusableElements(container);
		if (elements.length === 0) {
			event.preventDefault();
			container.focus();
			return;
		}

		const first = elements[0];
		const last = elements[elements.length - 1];

		if (event.shiftKey && document.activeElement === first) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && document.activeElement === last) {
			event.preventDefault();
			first.focus();
		}
	}

	container.addEventListener('keydown', handleKeydown);

	return function releaseFocusTrap(): void {
		container.removeEventListener('keydown', handleKeydown);
		if (!hadTabIndex) container.removeAttribute('tabindex');
		previouslyFocused?.focus();
	};
}
