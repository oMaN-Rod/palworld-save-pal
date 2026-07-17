import { MessageType } from '$types';
import { sendAndWait } from '$lib/utils/websocketUtils';
import { getModalState } from '$states';
import { technologiesData } from '$lib/data';
import WorldOptionModal from './WorldOptionModal.svelte';

export type WorldOptionEntry = {
	key: string;
	kind: string;
	value: boolean | number | string | string[];
};

export type WorldOptionPayload = {
	present: boolean;
	version: number;
	settings: WorldOptionEntry[];
};

type PatchResult = { entries: { key: string; value: unknown }[] } | null;

/** Fetches the current WorldOption, opens the editor, and sends back only the
 *  changed keys. No-op when the save carries no WorldOption. */
export async function openWorldOptionModal(): Promise<void> {
	const payload = await sendAndWait<WorldOptionPayload>(MessageType.GET_WORLD_OPTION);
	if (!payload?.present) return;

	// technologiesData is populated during app bootstrap (see data/bootstrap.ts),
	// so this is already loaded by the time a page can show the WorldOption button.
	const technologies = Object.keys(technologiesData.technologies);

	const modal = getModalState();
	// @ts-ignore -- Svelte component types don't satisfy Component; every call site does this.
	const result = await modal.showModal<PatchResult>(WorldOptionModal, {
		title: 'World Options',
		settings: payload.settings,
		technologies
	});
	if (!result) return;

	await sendAndWait(MessageType.UPDATE_WORLD_OPTION, { entries: result.entries });
}
