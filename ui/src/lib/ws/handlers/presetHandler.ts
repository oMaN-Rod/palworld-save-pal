import { presetsData } from '$lib/data';
import { getToastState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

export const exportPresetHandler: WSMessageHandler = {
	type: MessageType.EXPORT_PRESET,
	async handle(data: { message: string; file_path: string }) {
		const toast = getToastState();
		toast.add(data.message, 'Export Success', 'success');
	}
};

export const exportPresetsHandler: WSMessageHandler = {
	type: MessageType.EXPORT_PRESETS,
	async handle(data: { message: string; file_path: string }) {
		const toast = getToastState();
		toast.add(data.message, 'Export Success', 'success');
	}
};

export const importPresetHandler: WSMessageHandler = {
	type: MessageType.IMPORT_PRESET,
	async handle(data: { message: string; count: number }) {
		const toast = getToastState();
		toast.add(data.message, 'Import Success', 'success');
		// Refresh the presets data to include the new imported preset
		await presetsData.reset();
	}
};

export const presetHandlers = [exportPresetHandler, exportPresetsHandler, importPresetHandler];
