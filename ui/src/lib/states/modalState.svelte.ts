import { ConfirmModal } from '$components';
import type { Component } from 'svelte';

export function createModalState() {
	let isOpen = $state(false);
	let component: Component | null = $state(null);
	let props = $state<Record<string, any>>({});
	let resolveModal: ((value: any) => void) | null = $state(null);

	function showModal<T>(
		modalComponent: Component,
		modalProps: Record<string, any> = {}
	): Promise<T> {
		return new Promise((resolve) => {
			component = modalComponent;
			props = modalProps;
			isOpen = true;
			resolveModal = resolve as (value: any) => void;
		});
	}

	function showConfirmModal(options: {
		title?: string;
		message?: string;
		confirmText?: string;
		cancelText?: string;
	}): Promise<boolean> {
		// @ts-ignore
		return showModal(ConfirmModal, options);
	}

	function closeModal(value?: any) {
		isOpen = false;
		component = null;
		props = {};
		if (resolveModal) {
			resolveModal(value);
			resolveModal = null;
		}
	}

	return {
		get isOpen() {
			return isOpen;
		},
		get component() {
			return component;
		},
		get props() {
			return props;
		},
		showModal,
		showConfirmModal,
		closeModal
	};
}

let modalState: ReturnType<typeof createModalState>;

export function getModalState() {
	if (!modalState) {
		modalState = createModalState();
	}
	return modalState;
}
