import { MessageType } from '$types';
import { sendAndWait } from '$lib/utils/websocketUtils';
import { getModalState } from '$states';
import { technologiesData } from '$lib/data/technologies.svelte';
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

export async function openWorldOptionModal(): Promise<void> {
	try {
		const payload = await sendAndWait<WorldOptionPayload>(MessageType.GET_WORLD_OPTION);
		if (!payload?.present) return;

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
	} catch (error) {
		console.error('Error opening world options:', error);
	}
}
