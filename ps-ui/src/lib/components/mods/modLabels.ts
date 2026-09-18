import * as m from '$i18n/messages';

const typeLabels: Record<string, () => string> = {
	ue4ss: m.mods_panel_type_ue4ss,
	palschema: m.mods_panel_type_palschema,
	pak: m.mods_panel_type_pak,
	logicmods: m.mods_panel_type_logicmods,
	nativedll: m.mods_panel_type_nativedll,
	workshop: m.mods_panel_type_workshop,
	hybrid: m.mods_panel_type_hybrid,
	framework: m.mods_panel_type_framework
};

const destinationLabels: Record<string, () => string> = {
	ue4ss: m.mods_review_dest_ue4ss,
	pak: m.mods_review_dest_pak,
	logicmods: m.mods_review_dest_logicmods,
	nativedll: m.mods_review_dest_nativedll,
	palschema: m.mods_review_dest_palschema,
	workshop: m.mods_review_dest_workshop,
	passthrough: m.mods_review_dest_passthrough,
	framework: m.mods_panel_type_framework
};

export function modTypeLabel(modType: string): string {
	return typeLabels[modType]?.() ?? modType;
}

export function destinationLabel(kind: string): string {
	return destinationLabels[kind]?.() ?? kind;
}
