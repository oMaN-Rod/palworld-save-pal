// A 3 m Pal projects to 0.11 px at zoom 0 and 13.6 px at zoom 7 -- below the
// threshold the scenery layer culls at -- so true scale is the floor of the
// range rather than a usable default. The mapping is logarithmic so the
// resolution sits around the default instead of in the invisible bottom end.
import { sliderToScale as toScale, scaleToSlider as toSlider } from './logScale';

export const PAL_SCALE_MIN = 1;
export const PAL_SCALE_MAX = 60;
export const PAL_SCALE_DEFAULT = 30;

export function sliderToScale(position: number): number {
	return toScale(position, PAL_SCALE_MIN, PAL_SCALE_MAX);
}

export function scaleToSlider(scale: number): number {
	return toSlider(scale, PAL_SCALE_MIN, PAL_SCALE_MAX);
}
