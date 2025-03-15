import type { ToastPosition, ToastTransition, ToastType } from '$types';

class ToastState {
	toasts = $state<ToastType[]>([]);
	position = $state<ToastPosition>('top-right');
	transition = $state<ToastTransition>({ type: 'fly', params: { x: 300 } });
	private timeouts = new Map<string, ReturnType<typeof setTimeout>>();

	setPosition(position: ToastPosition) {
		this.position = position;
	}

	setTransition(transition: ToastTransition) {
		this.transition = transition;
	}

	add(
		message: string,
		title: string | undefined = undefined,
		color: ToastType['color'] = 'default',
		durationMs = 5000
	) {
		const id = crypto.randomUUID();
		this.toasts = [...this.toasts, { id, title, message, color }];

		const timeoutId = setTimeout(() => {
			this.remove(id);
		}, durationMs);

		this.timeouts.set(id, timeoutId);
	}

	remove(id: string) {
		this.toasts = this.toasts.filter((toast) => toast.id !== id);
		const timeoutId = this.timeouts.get(id);
		if (timeoutId) {
			clearTimeout(timeoutId);
			this.timeouts.delete(id);
		}
	}
}

const toastState = new ToastState();
export const getToastState = () => toastState;
