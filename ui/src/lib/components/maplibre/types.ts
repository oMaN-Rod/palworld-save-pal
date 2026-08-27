export type ControlPosition = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

export type Theme = 'light' | 'dark' | 'auto';

export interface TooltipFeature {
	properties: Record<string, any>;
	layer: string;
	lngLat: { lng: number; lat: number };
}

export type MeasureMode = 'none' | 'line' | 'polygon';
export type DrawMode = 'point' | 'line' | 'polygon' | 'rectangle' | 'circle';
export type EditMode = 'select' | 'drag' | 'modify' | 'delete';

export type MeasurementUnit = 'ft' | 'mi' | 'm' | 'km';
export type PointStyle = 'marker' | 'vertex';

export interface FeatureStyleProperties {
	strokeColor?: string;
	fillColor?: string;
	strokeWidth?: number;
	lineDash?: number[];
	markerColor?: string;
	markerIcon?: string;
	markerIconSource?: 'builtin' | 'iconify';
}

export interface FeatureMetadata {
	name?: string;
	description?: string;
	createdBy?: string;
	createdAt?: string;
}
