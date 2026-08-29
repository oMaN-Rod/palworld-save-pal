import type { TourDefinition } from './types';
import { navigationTour } from './navigationTour';
import { editTour } from './editTour';

export const tours: TourDefinition[] = [navigationTour, editTour];

export type { TourDefinition };
