// Deep import: this store sits in the root layout graph, and the $components
// barrel (or even the modals barrel) would drag the heavy modal leaves with it.
import ConfirmModal from '$components/modals/confirm/ConfirmModal.svelte';
import type { Component } from 'svelte';

export interface ModalEntry {
	id: number;
	component: Component;
	props: Record<string, any>;
}

class ModalState {
	#stack = $state<ModalEntry[]>([]);
	#resolvers = new Map<number, (value: any) => void>();
	#nextId = 0;

	constructor() {
		this.closeModal = this.closeModal.bind(this);
		this.closeEntry = this.closeEntry.bind(this);
		this.showModal = this.showModal.bind(this);
		this.showConfirmModal = this.showConfirmModal.bind(this);
	}

	showModal<T>(modalComponent: Component, modalProps: Record<string, any> = {}): Promise<T> {
		const id = ++this.#nextId;
		return new Promise((resolve) => {
			this.#resolvers.set(id, resolve as (value: any) => void);
			this.#stack = [...this.#stack, { id, component: modalComponent, props: modalProps }];
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
		const top = this.#stack[this.#stack.length - 1];
		if (top) this.closeEntry(top.id, value);
	}

	closeEntry(id: number, value?: any) {
		const index = this.#stack.findIndex((entry) => entry.id === id);
		if (index === -1) return;
		const removed = this.#stack.slice(index);
		this.#stack = this.#stack.slice(0, index);
		for (let i = removed.length - 1; i >= 0; i--) {
			const resolve = this.#resolvers.get(removed[i].id);
			this.#resolvers.delete(removed[i].id);
			resolve?.(removed[i].id === id ? value : undefined);
		}
	}

	get stack() {
		return this.#stack;
	}

	get isOpen() {
		return this.#stack.length > 0;
	}

	get component() {
		return this.#stack[this.#stack.length - 1]?.component ?? null;
	}

	get props() {
		return this.#stack[this.#stack.length - 1]?.props ?? {};
	}
}
const modalStateInstance = new ModalState();
export const getModalState = () => modalStateInstance;
