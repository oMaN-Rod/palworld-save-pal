import type { DriveStep } from 'driver.js';

export interface TourCheckpoint {
	selector?: string;
	event?: string;
	condition?: () => boolean;
	advanceDelayMs?: number;
}

export interface TourStep extends DriveStep {
	checkpoint?: TourCheckpoint;
}

export interface TourDefinition {
	id: string;
	title: string;
	description: string;
	route: string;
	requiresSaveFile: boolean;
	steps: TourStep[];
}
