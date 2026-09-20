// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';

import { trapFocus } from './focusTrap';

function mountContainer(html: string): HTMLElement {
	const container = document.createElement('div');
	container.innerHTML = html;
	document.body.appendChild(container);
	return container;
}

afterEach(() => {
	document.body.innerHTML = '';
});

describe('trapFocus', () => {
	it('moves focus to the first focusable element', () => {
		const container = mountContainer('<button>First</button><button>Second</button>');

		trapFocus(container);

		expect(document.activeElement).toBe(container.querySelector('button'));
	});

	it('focuses the container itself when nothing inside is focusable', () => {
		const container = mountContainer('<p>No controls here</p>');

		trapFocus(container);

		expect(document.activeElement).toBe(container);
	});

	it('wraps Tab from the last element to the first', () => {
		const container = mountContainer('<button>First</button><button>Second</button>');
		trapFocus(container);

		const [first, second] = Array.from(container.querySelectorAll('button'));
		second.focus();

		const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
		container.dispatchEvent(event);

		expect(document.activeElement).toBe(first);
		expect(event.defaultPrevented).toBe(true);
	});

	it('wraps Shift+Tab from the first element to the last', () => {
		const container = mountContainer('<button>First</button><button>Second</button>');
		trapFocus(container);

		const [first, second] = Array.from(container.querySelectorAll('button'));
		first.focus();

		const event = new KeyboardEvent('keydown', {
			key: 'Tab',
			shiftKey: true,
			bubbles: true,
			cancelable: true
		});
		container.dispatchEvent(event);

		expect(document.activeElement).toBe(second);
		expect(event.defaultPrevented).toBe(true);
	});

	it('leaves non-Tab keys alone', () => {
		const container = mountContainer('<button>First</button><button>Second</button>');
		trapFocus(container);

		const second = container.querySelectorAll('button')[1];
		second.focus();

		const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
		container.dispatchEvent(event);

		expect(document.activeElement).toBe(second);
		expect(event.defaultPrevented).toBe(false);
	});

	it('restores focus to the previously focused element on release', () => {
		const trigger = document.createElement('button');
		trigger.textContent = 'Open';
		document.body.appendChild(trigger);
		trigger.focus();

		const container = mountContainer('<button>Inside</button>');
		const release = trapFocus(container);
		release();

		expect(document.activeElement).toBe(trigger);
	});
});
