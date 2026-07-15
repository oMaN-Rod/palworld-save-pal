import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockAppState = { selectedPal: undefined as unknown, saveState: vi.fn() };
vi.mock('./appState.svelte', () => ({
	getAppState: () => mockAppState
}));

import { getPalEditorState } from './palEditorState.svelte';

const pal = { instance_id: 'abc' } as never;

beforeEach(() => {
	mockAppState.selectedPal = undefined;
	mockAppState.saveState = vi.fn();
});

describe('palEditorState', () => {
	it('open(pal) selects the pal and opens without loading', () => {
		const editor = getPalEditorState();
		editor.open(pal);
		expect(mockAppState.selectedPal).toBe(pal);
		expect(editor.isOpen).toBe(true);
		expect(editor.loading).toBe(false);
	});

	it('openLoading() opens in the loading state', () => {
		const editor = getPalEditorState();
		editor.openLoading();
		expect(editor.isOpen).toBe(true);
		expect(editor.loading).toBe(true);
	});

	it('resolve(pal) selects the pal and clears loading', () => {
		const editor = getPalEditorState();
		editor.openLoading();
		editor.resolve(pal);
		expect(mockAppState.selectedPal).toBe(pal);
		expect(editor.loading).toBe(false);
	});

	it('close() saves and resets isOpen/loading', () => {
		const editor = getPalEditorState();
		editor.open(pal);
		editor.close();
		expect(mockAppState.saveState).toHaveBeenCalledTimes(1);
		expect(editor.isOpen).toBe(false);
		expect(editor.loading).toBe(false);
	});
});
