// @vitest-environment jsdom
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LiveInstanceModal from '../LiveInstanceModal.svelte';
import LiveInstanceSwitcher from '../LiveInstanceSwitcher.svelte';

const INSTANCES = [
	{
		id: 'auto:11',
		source: 'auto' as const,
		name: 'Solo World',
		host: '127.0.0.1',
		port: 52104,
		live: true
	},
	{
		id: 'saved:1',
		source: 'saved' as const,
		name: 'Remote box',
		host: '10.0.0.14',
		port: 8788,
		live: false
	}
];

describe('LiveInstanceSwitcher', () => {
	it('renders every instance with its address', () => {
		const { getByText } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		expect(getByText('Solo World')).toBeTruthy();
		expect(getByText(/127\.0\.0\.1:52104/)).toBeTruthy();
		expect(getByText('Remote box')).toBeTruthy();
		expect(getByText(/10\.0\.0\.14:8788/)).toBeTruthy();
	});

	it('marks the active instance', () => {
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'saved:1',
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		const active = container.querySelector('[data-instance-id="saved:1"]');
		expect(active?.getAttribute('aria-current')).toBe('true');
	});

	it('calls onselect with the clicked id', async () => {
		const onselect = vi.fn();
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				onselect,
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		const button = container.querySelector('[data-instance-id="saved:1"]') as HTMLElement;
		button.click();
		await tick();
		expect(onselect).toHaveBeenCalledWith('saved:1');
	});

	it('offers edit only for saved instances', () => {
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		expect(container.querySelector('[data-edit-id="saved:1"]')).toBeTruthy();
		expect(container.querySelector('[data-edit-id="auto:11"]')).toBeNull();
	});

	it('renders an empty state with nothing detected or saved', () => {
		const { getByText } = render(LiveInstanceSwitcher, {
			props: { instances: [], activeId: null, onselect: vi.fn(), onedit: vi.fn(), onadd: vi.fn() }
		});
		expect(getByText('No instances found')).toBeTruthy();
	});

	it('marks the active auto-discovered instance disconnected once the bridge drops, not live', () => {
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				activeConnected: false,
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		const dot = container.querySelector('[data-instance-id="auto:11"] [data-dot-status]');
		expect(dot?.getAttribute('data-dot-status')).toBe('disconnected');
	});

	it('marks the active instance connected when the bridge is up', () => {
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				activeConnected: true,
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		const dot = container.querySelector('[data-instance-id="auto:11"] [data-dot-status]');
		expect(dot?.getAttribute('data-dot-status')).toBe('connected');
	});

	it('marks a saved, inactive instance unknown rather than live or dead', () => {
		const { container } = render(LiveInstanceSwitcher, {
			props: {
				instances: INSTANCES,
				activeId: 'auto:11',
				activeConnected: true,
				onselect: vi.fn(),
				onedit: vi.fn(),
				onadd: vi.fn()
			}
		});
		const dot = container.querySelector('[data-instance-id="saved:1"] [data-dot-status]');
		expect(dot?.getAttribute('data-dot-status')).toBe('unknown');
	});
});

describe('LiveInstanceModal', () => {
	const initial = { name: '', host: '127.0.0.1', port: 8788, token: '' };

	it('blocks save until host, port and token are filled', async () => {
		const onsave = vi.fn();
		const { container } = render(LiveInstanceModal, {
			props: { initial, ontest: vi.fn(), onsave, oncancel: vi.fn() }
		});
		(container.querySelector('[data-action="save"]') as HTMLElement).click();
		await tick();
		expect(onsave).not.toHaveBeenCalled();
	});

	it('warns when the address is not loopback', async () => {
		const { getByText } = render(LiveInstanceModal, {
			props: {
				initial: { name: 'Remote', host: '10.0.0.14', port: 8788, token: 's3cr3t' },
				ontest: vi.fn(),
				onsave: vi.fn(),
				oncancel: vi.fn()
			}
		});
		expect(getByText(/not encrypted/)).toBeTruthy();
	});

	it('surfaces a failed test result', async () => {
		const ontest = vi.fn().mockResolvedValue({ ok: false, error: 'timeout' });
		const { container, getByText } = render(LiveInstanceModal, {
			props: {
				initial: { name: 'Dead', host: '127.0.0.1', port: 1, token: 't' },
				ontest,
				onsave: vi.fn(),
				oncancel: vi.fn()
			}
		});
		(container.querySelector('[data-action="test"]') as HTMLElement).click();
		await tick();
		await tick();
		expect(getByText('Could not connect')).toBeTruthy();
	});
});
