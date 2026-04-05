import { driver, type Driver } from 'driver.js';
import 'driver.js/dist/driver.css';
import { goto } from '$app/navigation';
import type { TourDefinition } from './types';
import { tours } from './index';

class TourService {
	activeTourId = $state<string | null>(null);
	#driver: Driver | null = null;

	async startTour(tourId: string): Promise<void> {
		const tour = tours.find((t) => t.id === tourId);
		if (!tour) return;

		this.activeTourId = tourId;

		await goto(tour.route);

		// Wait for route transition and DOM to settle
		await new Promise((resolve) => setTimeout(resolve, 300));

		this.#driver = driver({
			showProgress: true,
			animate: true,
			overlayColor: 'rgba(0, 0, 0, 0.7)',
			stagePadding: 8,
			stageRadius: 8,
			popoverClass: 'driver-popover',
			steps: tour.steps,
			onDestroyed: () => {
				this.activeTourId = null;
				this.#driver = null;
			}
		});

		this.#driver.drive();
	}

	stopTour(): void {
		if (this.#driver) {
			this.#driver.destroy();
			this.#driver = null;
		}
		this.activeTourId = null;
	}

	get isActive(): boolean {
		return this.activeTourId !== null;
	}
}

export const tourService = new TourService();
