import { sliderToScale as toScale, scaleToSlider as toSlider } from './logScale';

export const MAP_OBJECT_SCALE_MIN = 1;
export const MAP_OBJECT_SCALE_MAX = 60;
export const MAP_OBJECT_SCALE_DEFAULT = 20;
export const MAP_OBJECT_WATCHTOWER_SCALE_DEFAULT = 5;

export function sliderToScale(position: number): number {
	return toScale(position, MAP_OBJECT_SCALE_MIN, MAP_OBJECT_SCALE_MAX);
}

export function scaleToSlider(scale: number): number {
	return toSlider(scale, MAP_OBJECT_SCALE_MIN, MAP_OBJECT_SCALE_MAX);
}
