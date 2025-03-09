import { ConfirmModal } from '$components';
import type { Component } from 'svelte';

class ModalState {
	#isOpen = $state(false);
	#component = $state<Component | null>(null);
	#props = $state<Record<string, any>>({});
	#resolveModal = $state<((value: any) => void) | null>(null);

	showModal<T>(modalComponent: Component, modalProps: Record<string, any> = {}): Promise<T> {
		return new Promise((resolve) => {
			this.#component = modalComponent;
			this.#props = modalProps;
			this.#isOpen = true;
			this.#resolveModal = resolve as (value: any) => void;
		});
	}

	showConfirmModal(options: {
		title?: string;
		message?: string;
		confirmText?: string;
		cancelText?: string;
	}): Promise<boolean> {
		// @ts-ignore
		return this.showModal<boolean>(ConfirmModal, options);
	}

	closeModal(value?: any) {
		this.#isOpen = false;
		this.#component = null;
		this.#props = {};
		if (this.#resolveModal) {
			this.#resolveModal(value);
			this.#resolveModal = null;
		}
	}

	get isOpen() {
		return this.#isOpen;
	}

	get component() {
		return this.#component;
	}

	get props() {
		return this.#props;
	}
}
const modalStateInstance = new ModalState();
export const getModalState = () => modalStateInstance;
