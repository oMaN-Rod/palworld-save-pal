// @vitest-environment jsdom
import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

type MonacoProps = { options?: { minimap?: { enabled: boolean }; automaticLayout?: boolean } };

const { monacoProps } = vi.hoisted(() => ({ monacoProps: [] as MonacoProps[] }));

vi.mock('$components/ui/monaco/Monaco.svelte', () => ({
	default: (_anchor: unknown, props: MonacoProps) => {
		monacoProps.push(props);
	}
}));
vi.mock('$components/ui/monaco/paletteTheme', () => ({
	buildEditorTheme: () => ({}),
	EDITOR_THEME_NAME: 'ps'
}));
vi.mock('$lib/components/seo', () => ({ Seo: () => {} }));
vi.mock('$env/static/public', () => ({ PUBLIC_DESKTOP_MODE: 'false' }));
vi.mock('$lib/utils/websocketUtils', () => ({ sendAndWait: vi.fn() }));
vi.mock('$lib/data/convertSav', () => ({
	savToJson: vi.fn().mockResolvedValue('{"a":1}'),
	jsonToSav: vi.fn()
}));
vi.mock('$states', () => ({
	getAppState: () => ({ settings: {} }),
	getToastState: () => ({ add: vi.fn() }),
	theme: { current: 'dark' }
}));

// The stub must be installed before the page pulls in the layout store.
const { setViewport } = installViewportStub();
const { default: EditorPage } = await import('../+page.svelte');

async function openASave() {
	const input = document.querySelector('input[type="file"]') as HTMLInputElement;
	const file = new File([new Uint8Array([1, 2, 3])], 'Level.sav');
	await fireEvent.change(input, { target: { files: [file] } });
	await waitFor(() => expect(monacoProps.length).toBeGreaterThan(0));
}

beforeEach(() => {
	vi.clearAllMocks();
	monacoProps.length = 0;
	setViewport(1440);
});

describe('editor page', () => {
	it('drops the minimap on a phone', async () => {
		setViewport(390);
		render(EditorPage);
		await tick();
		await openASave();

		expect(monacoProps[0].options?.minimap).toEqual({ enabled: false });
	});

	it('keeps the minimap on a desktop', async () => {
		render(EditorPage);
		await tick();
		await openASave();

		expect(monacoProps[0].options?.minimap).toEqual({ enabled: true });
	});

	it('still lays the editor out on its own', async () => {
		render(EditorPage);
		await tick();
		await openASave();

		expect(monacoProps[0].options?.automaticLayout).toBe(true);
	});

	it('gives the drop target the width of the screen on a phone', async () => {
		setViewport(390);
		render(EditorPage);
		await tick();

		const dropzone = document.querySelector('#editor-dropzone');
		expect(dropzone).not.toBeNull();
		expect(dropzone?.className).toContain('w-full');
		const unprefixed = (dropzone?.className ?? '')
			.split(/ +/)
			.filter((token) => !token.includes(':'));
		expect(unprefixed).not.toContain('w-1/2');
	});

	it('wraps the toolbar rather than running it off the edge', async () => {
		setViewport(390);
		render(EditorPage);
		await tick();
		await openASave();

		const toolbar = document.querySelector('#editor-toolbar');
		expect(toolbar?.className).toContain('flex-wrap');
	});
});
