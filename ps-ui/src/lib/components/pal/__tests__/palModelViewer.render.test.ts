// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type RendererRecord = { disposed: number; contextLost: number };

const { renderers, meshLibrary } = vi.hoisted(() => ({
	renderers: [] as { disposed: number; contextLost: number }[],
	meshLibrary: {
		requested: [] as string[],
		url: (key: string) => (key === 'missing' ? null : `/models/pals/${key}.glb`)
	}
}));

// jsdom has no GL context, so the renderer is counted, not run.
vi.mock('three', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	class FakeRenderer {
		record = { disposed: 0, contextLost: 0 };
		constructor() {
			renderers.push(this.record);
		}
		dispose() {
			this.record.disposed += 1;
		}
		forceContextLoss() {
			this.record.contextLost += 1;
		}
		setClearAlpha() {}
		setPixelRatio() {}
		setSize() {}
		render() {}
	}
	return { ...actual, WebGLRenderer: FakeRenderer };
});

vi.mock('$components/map/scene/pal/palMeshLibrary', () => ({
	palModelUrl: (key: string) => meshLibrary.url(key),
	requestPalMesh: (key: string) => {
		meshLibrary.requested.push(key);
		return null;
	},
	palMeshFailed: () => false,
	onPalMeshLoaded: () => () => {}
}));

const { default: PalModelViewer } = await import('../PalModelViewer.svelte');

function canvas(): HTMLElement | null {
	return document.querySelector('canvas');
}

function loadButton(): HTMLElement {
	return screen.getByRole('button', { name: 'Load 3D model' });
}

beforeEach(() => {
	renderers.length = 0;
	meshLibrary.requested.length = 0;
	// jsdom has no ResizeObserver, and the viewer observes its canvas.
	vi.stubGlobal(
		'ResizeObserver',
		class {
			observe() {}
			disconnect() {}
		}
	);
});

describe('PalModelViewer', () => {
	it('offers to load the model instead of loading it', async () => {
		render(PalModelViewer, { props: { characterKey: 'testpal' } });
		await tick();

		expect(loadButton()).toBeTruthy();
		expect(canvas()).toBeNull();
		expect(renderers).toHaveLength(0);
		expect(meshLibrary.requested).toEqual([]);
	});

	it('mounts the model once the user asks for it', async () => {
		const user = userEvent.setup();
		render(PalModelViewer, { props: { characterKey: 'testpal' } });
		await tick();

		await user.click(loadButton());
		await tick();

		expect(canvas()).not.toBeNull();
		expect(renderers).toHaveLength(1);
		expect(meshLibrary.requested).toEqual(['testpal']);
	});

	it('gives up its GL context when it goes away', async () => {
		const user = userEvent.setup();
		const { unmount } = render(PalModelViewer, { props: { characterKey: 'testpal' } });
		await tick();

		await user.click(loadButton());
		await tick();

		unmount();
		await tick();

		expect(renderers[0]).toMatchObject<RendererRecord>({ disposed: 1, contextLost: 1 });
	});

	it('asks again for a different pal', async () => {
		const user = userEvent.setup();
		const { rerender } = render(PalModelViewer, { props: { characterKey: 'testpal' } });
		await tick();

		await user.click(loadButton());
		await tick();
		expect(canvas()).not.toBeNull();

		await rerender({ characterKey: 'otherpal' });
		await tick();

		expect(canvas()).toBeNull();
		expect(loadButton()).toBeTruthy();
		expect(renderers[0].disposed).toBe(1);
	});

	it('shows the fallback rather than an offer for a pal with no model', async () => {
		render(PalModelViewer, { props: { characterKey: 'missing' } });
		await tick();

		expect(screen.queryByRole('button', { name: 'Load 3D model' })).toBeNull();
		expect(canvas()).toBeNull();
	});
});
