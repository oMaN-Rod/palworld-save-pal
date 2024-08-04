import type { FlyParams, ScaleParams } from 'svelte/transition';

export type ToastType = {
	id: string;
	title?: string;
	message: string;
	color?: 'default' | 'success' | 'error' | 'warning' | 'info';
};

export type ToastPosition = 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left';

export type ToastTransition = {
	type: 'fly' | 'scale';
	params?: FlyParams | ScaleParams;
};
