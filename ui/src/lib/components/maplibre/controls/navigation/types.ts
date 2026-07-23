import type { ControlPosition } from '../../types.js';

export interface NavigationControlProps {
	position?: ControlPosition;
	showCompass?: boolean;
	showZoom?: boolean;
	visualizePitch?: boolean;
	visualizeRoll?: boolean;
}
