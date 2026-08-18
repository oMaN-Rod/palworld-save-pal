import { editTour } from './editTour';
import { navigationTour } from './navigationTour';
import type { TourDefinition } from './types';

export const tours: TourDefinition[] = [navigationTour, editTour];

export type { TourDefinition };
