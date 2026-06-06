import { driver, type Driver, type DriveStep } from 'driver.js';
import 'driver.js/dist/driver.css';
import { goto } from '$app/navigation';
import type { TourStep } from './types';
import { tours } from './index';

class TourService {
	activeTourId = $state<string | null>(null);
	#driver: Driver | null = null;
	#checkpointCleanup: (() => void) | null = null;

	async startTour(tourId: string): Promise<void> {
		const tour = tours.find((t) => t.id === tourId);
		if (!tour) return;

		this.activeTourId = tourId;

		await goto(tour.route);

		// Wait for route transition and DOM to settle
		await new Promise((resolve) => setTimeout(resolve, 300));

		const steps = tour.steps.map((step) => this.#prepareStep(step));

		this.#driver = driver({
			showProgress: true,
			animate: true,
			overlayColor: 'rgba(0, 0, 0, 0.7)',
			stagePadding: 8,
			stageRadius: 8,
			popoverClass: 'driver-popover',
			steps,
			onDestroyed: () => {
				this.#clearCheckpoint();
				this.activeTourId = null;
				this.#driver = null;
			}
		});

		this.#driver.drive();
	}

	stopTour(): void {
		this.#clearCheckpoint();
		if (this.#driver) {
			this.#driver.destroy();
			this.#driver = null;
		}
		this.activeTourId = null;
	}

	get isActive(): boolean {
		return this.activeTourId !== null;
	}

	#prepareStep(step: TourStep): DriveStep {
		const { checkpoint, ...base } = step;
		if (!checkpoint) return base;

		const { selector, event = 'click', condition, advanceDelayMs = 100 } = checkpoint;
		const userHighlighted = base.onHighlighted;
		const userDeselected = base.onDeselected;

		return {
			...base,
			onHighlighted: (element, stepInfo, opts) => {
				this.#clearCheckpoint();
				const cleanups: (() => void)[] = [];
				let advanced = false;
				const advance = () => {
					if (advanced) return;
					advanced = true;
					this.#clearCheckpoint();
					setTimeout(() => this.#driver?.moveNext(), advanceDelayMs);
				};

				const initiallySatisfied = condition ? condition() : false;
				const blockNext = !!selector || (!!condition && !initiallySatisfied);

				if (blockNext) {
					const nextBtn = document.querySelector<HTMLButtonElement>(
						'.driver-popover-next-btn'
					);
					if (nextBtn) {
						const originalDisplay = nextBtn.style.display;
						nextBtn.style.display = 'none';
						cleanups.push(() => {
							nextBtn.style.display = originalDisplay;
						});
					}
				}

				if (selector) {
					const target = document.querySelector(selector);
					if (target) {
						const handler = () => advance();
						target.addEventListener(event, handler, { once: true });
						cleanups.push(() => target.removeEventListener(event, handler));
					}
				}

				if (condition && !initiallySatisfied) {
					const stopRoot = $effect.root(() => {
						$effect(() => {
							if (condition()) advance();
						});
					});
					cleanups.push(stopRoot);
				}

				this.#checkpointCleanup = () => {
					for (const fn of cleanups) fn();
				};

				userHighlighted?.(element, stepInfo, opts);
			},
			onDeselected: (element, stepInfo, opts) => {
				this.#clearCheckpoint();
				userDeselected?.(element, stepInfo, opts);
			}
		};
	}

	#clearCheckpoint(): void {
		if (this.#checkpointCleanup) {
			this.#checkpointCleanup();
			this.#checkpointCleanup = null;
		}
	}
}

export const tourService = new TourService();
