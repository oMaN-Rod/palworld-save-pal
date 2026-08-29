import { getToastState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

type ExportFile = { name: string; content: string };
type ExportResult = { message: string; file_path: string };

export function browserDownload(name: string, base64: string): void {
	const binary = atob(base64);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	const blob = new Blob([bytes], { type: 'application/octet-stream' });
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = name;
	a.click();
	URL.revokeObjectURL(url);
}

export function handleExportFrame(
	data: ExportFile[] | ExportResult,
	deps: { download: (name: string, content: string) => void; toast: (message: string) => void }
): void {
	if (Array.isArray(data)) {
		for (const { name, content } of data) deps.download(name, content);
	} else if (data && typeof data === 'object' && 'file_path' in data) {
		deps.toast(data.message);
	}
}

export const exportBlueprintFileHandler: WSMessageHandler = {
	type: MessageType.EXPORT_BLUEPRINT_FILE,
	async handle(data) {
		const toast = getToastState();
		handleExportFrame(data, {
			download: browserDownload,
			toast: (message) => toast.add(message, 'Blueprint exported', 'success')
		});
	}
};

export const blueprintHandlers = [exportBlueprintFileHandler];
