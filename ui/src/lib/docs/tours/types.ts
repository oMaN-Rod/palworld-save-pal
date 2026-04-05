import type { DriveStep } from 'driver.js';

export interface TourDefinition {
	id: string;
	title: string;
	description: string;
	route: string;
	requiresSaveFile: boolean;
	steps: DriveStep[];
}
