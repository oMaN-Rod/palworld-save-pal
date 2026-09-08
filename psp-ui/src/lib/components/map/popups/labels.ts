import * as m from '$i18n/messages';

/** Fallback title for feature types without a dedicated hover/popup: `fast_travel` -> `Fast Travel`. */
export function featureTypeLabel(type: string): string {
	return type
		.split('_')
		.filter((word) => word.length > 0)
		.map((word) => word[0].toUpperCase() + word.slice(1))
		.join(' ');
}

const LIVE_KIND_LABELS: Record<string, () => string> = {
	player: m.live_kind_player,
	otomo: m.live_kind_otomo,
	basepal: m.live_kind_basepal,
	wild: m.live_kind_wild,
	npc: m.live_kind_npc,
	palbox: m.live_kind_palbox
};

export function liveKindLabel(kind: string): string {
	return LIVE_KIND_LABELS[kind]?.() ?? featureTypeLabel(kind);
}
