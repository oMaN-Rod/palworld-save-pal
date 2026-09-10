import type { ControlPosition } from '../../types.js';

export interface FullscreenControlProps {
	position?: ControlPosition;
	container?: HTMLElement;
	pseudo?: boolean;
	onfullscreenstart?: () => void;
	onfullscreenend?: () => void;
}
