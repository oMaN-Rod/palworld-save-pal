import type { Pal } from '$types';
import { getAppState } from './appState.svelte';

class PalEditorState {
	#isOpen = $state(false);
	#loading = $state(false);

	constructor() {
		this.open = this.open.bind(this);
		this.openLoading = this.openLoading.bind(this);
		this.resolve = this.resolve.bind(this);
		this.close = this.close.bind(this);
	}

	open(pal?: Pal) {
		if (pal) getAppState().selectedPal = pal;
		this.#loading = false;
		this.#isOpen = true;
	}

	openLoading() {
		this.#loading = true;
		this.#isOpen = true;
	}

	resolve(pal: Pal) {
		getAppState().selectedPal = pal;
		this.#loading = false;
	}

	close() {
		getAppState().saveState();
		this.#isOpen = false;
		this.#loading = false;
	}

	get isOpen() {
		return this.#isOpen;
	}

	get loading() {
		return this.#loading;
	}
}

const palEditorStateInstance = new PalEditorState();
export const getPalEditorState = () => palEditorStateInstance;
