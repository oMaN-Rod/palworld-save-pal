import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';
import type { BlueprintRow, CaptureBlueprintResponse, CaptureOptions } from '$types';

export type BlueprintFormat = 'psp' | 'json';

class Blueprints {
	rows: BlueprintRow[] = $state([]);
	current: CaptureBlueprintResponse | null = $state(null);

	async list(): Promise<BlueprintRow[]> {
		const response = await sendAndWait<{ blueprints: BlueprintRow[] }>(MessageType.LIST_BLUEPRINTS);
		this.rows = Array.isArray(response?.blueprints) ? response.blueprints : [];
		return this.rows;
	}

	async capture(
		baseId: string,
		options: CaptureOptions,
		name: string
	): Promise<CaptureBlueprintResponse> {
		const res = await sendAndWait<CaptureBlueprintResponse>(MessageType.CAPTURE_BASE_BLUEPRINT, {
			base_id: baseId,
			options,
			name
		});
		this.current = res;
		return res;
	}

	async store(handle: string): Promise<string> {
		const { id } = await sendAndWait<{ id: string }>(MessageType.STORE_BLUEPRINT, { handle });
		await this.list();
		return id;
	}

	async loadFromId(id: string): Promise<CaptureBlueprintResponse> {
		const res = await sendAndWait<CaptureBlueprintResponse>(MessageType.LOAD_BLUEPRINT, { id });
		this.current = res;
		return res;
	}

	async loadFromContent(
		content: string,
		format: BlueprintFormat
	): Promise<CaptureBlueprintResponse> {
		const res = await sendAndWait<CaptureBlueprintResponse>(MessageType.LOAD_BLUEPRINT, {
			content,
			format
		});
		this.current = res;
		return res;
	}

	exportFile(handle: string, format: BlueprintFormat): void {
		send(MessageType.EXPORT_BLUEPRINT_FILE, { handle, format });
	}

	async exportRow(id: string, format: BlueprintFormat): Promise<void> {
		const res = await this.loadFromId(id);
		this.exportFile(res.handle, format);
	}

	reset(): void {
		this.rows = [];
		this.current = null;
	}
}

export const blueprintsData = new Blueprints();
