import type { Offset, PositionAnchor, PaddingOptions } from 'maplibre-gl';
import type { Snippet } from 'svelte';

export interface PopupProps {
	lnglat?: [number, number];
	open?: boolean;
	offset?: Offset;
	anchor?: PositionAnchor;
	maxWidth?: string;
	closeButton?: boolean;
	closeOnClick?: boolean;
	closeOnMove?: boolean;
	focusAfterOpen?: boolean;
	className?: string;
	subpixelPositioning?: boolean;
	locationOccludedOpacity?: number | string;
	padding?: PaddingOptions;
	onopen?: () => void;
	onclose?: () => void;
	children?: Snippet;
}
