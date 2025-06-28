import L from 'leaflet';

const yx = L.latLng;

interface CoordinatesArray {
	[index: number]: number;
	length: number;
}

export const xy = function (x: number | CoordinatesArray, y?: number): L.LatLng {
	if (Array.isArray(x)) {
		return yx(x[1], x[0]);
	}
	return yx(y as number, x as number);
};
