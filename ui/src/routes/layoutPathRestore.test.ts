import { beforeEach, describe, expect, it, vi } from 'vitest';

const nav = vi.hoisted(() => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/navigation', () => ({ goto: nav.goto }));
vi.mock('$app/environment', () => ({ browser: true }));
// Mirrors SvelteKit's resolve(): base + resolve_route(id), and base is '' here
// because svelte.config.js configures no paths.base.
vi.mock('$app/paths', () => ({ resolve: (id: string) => id }));

import { load } from './+layout';

const run = (search: string) =>
	(load as (event: { url: URL }) => void)({
		url: new URL(`http://127.0.0.1:5174/${search}`)
	});

describe('?path= restore', () => {
	beforeEach(() => nav.goto.mockClear());

	// psp-server redirects any unmatched route to /?path=<encoded>. On desktop
	// that is how every non-prerendered route boots, wiki entity pages included.
	it('navigates to a same-origin path, not a protocol-relative one', () => {
		run('?path=/wiki/pals/sheepball');

		expect(nav.goto).toHaveBeenCalledWith('/wiki/pals/sheepball');
		const target = nav.goto.mock.calls[0][0] as unknown as string;
		expect(new URL(target, 'http://127.0.0.1:5174/').origin).toBe('http://127.0.0.1:5174');
	});

	it('decodes an encoded path before navigating', () => {
		run('?path=/edit/pal%2520box');

		expect(nav.goto).toHaveBeenCalledWith('/edit/pal%20box');
	});

	it('does nothing without a path parameter', () => {
		run('');

		expect(nav.goto).not.toHaveBeenCalled();
	});
});
