import * as m from '$i18n/messages';
import type { ModTarget } from '$types';

const hazardMessages: Record<string, () => string> = {
	ue4ss_dual_instance: m.mods_hazard_ue4ss_dual_instance,
	workshop_proxy_dll: m.mods_hazard_workshop_proxy_dll,
	amity_legacy_folder: m.mods_hazard_amity_legacy_folder
};

export function hazardText(code: string): string {
	return hazardMessages[code]?.() ?? code;
}

const platformLabels: Record<string, () => string> = {
	win64: m.mods_platform_win64,
	wingdk: m.mods_platform_wingdk,
	linux: m.mods_platform_linux,
	mac: m.mods_platform_mac
};

export function platformLabel(code: string): string {
	return platformLabels[code]?.() ?? code;
}

export function targetHazards(target: ModTarget): string[] {
	return target.detected?.hazards ?? [];
}

/** Some targets are stored forward-slashed, so roots compare blind to case and separators. */
export function normaliseRoot(path: string): string {
	return path.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

export function targetName(target: ModTarget, servers: { id: number; name: string }[]): string {
	if (target.kind !== 'server') return target.name;
	return servers.find((server) => server.id === target.server_id)?.name ?? target.name;
}

export async function confirmRemoveTarget(
	target: ModTarget,
	name: string,
	modal: {
		showConfirmModal(options: {
			title?: string;
			message?: string;
			confirmText?: string;
			cancelText?: string;
		}): Promise<boolean>;
	},
	mods: { removeTarget(targetId: string): void }
): Promise<void> {
	if (target.kind !== 'client') return;
	const confirmed = await modal.showConfirmModal({
		title: m.mods_target_remove_title(),
		message: m.mods_target_remove_message({ name }),
		confirmText: m.mods_target_remove(),
		cancelText: m.mods_panel_cancel()
	});
	if (confirmed) mods.removeTarget(target.id);
}
