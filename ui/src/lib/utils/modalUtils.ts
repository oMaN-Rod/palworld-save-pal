export function focusModal(container?: HTMLElement) {
	const root = container || document;

	setTimeout(() => {
		const selectors = [
			// Try combobox/select first
			'[role="combobox"]',
			'button[aria-expanded]',
			'.combobox button',
			'select:not([disabled])',
			// Then input fields
			'input:not([disabled]):not([readonly]):not([type="hidden"])',
			'textarea:not([disabled]):not([readonly])',
			// Finally primary button
			'[data-modal-primary]:not([disabled])'
		];

		for (const selector of selectors) {
			const element = root.querySelector(selector) as HTMLElement;
			if (element) {
				element.focus();
				// Select text in inputs for easy editing
				if (
					element instanceof HTMLInputElement &&
					(element.type === 'text' || element.type === 'number')
				) {
					element.select();
				}
				return;
			}
		}
	}, 0);
}
