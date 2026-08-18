import { send, sendAndWait } from '$lib/utils/websocketUtils';
import type {
	BlueprintGeometry,
	BlueprintRow,
	CaptureBlueprintResponse,
	CaptureOptions,
	PlaceBlueprintResponse,
	PlacementAnchor,
	ValidatePlacementResponse
} from '$types';
import { MessageType } from '$types';

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

	async remove(id: string): Promise<void> {
		await sendAndWait(MessageType.DELETE_BLUEPRINT, { id });
		await this.list();
	}

	async requestGeometry(handle: string): Promise<BlueprintGeometry> {
		return sendAndWait<BlueprintGeometry>(MessageType.REQUEST_BLUEPRINT_GEOMETRY, { handle });
	}

	async validate(
		handle: string,
		anchor: PlacementAnchor,
		targetGuild: string
	): Promise<ValidatePlacementResponse> {
		return sendAndWait<ValidatePlacementResponse>(MessageType.VALIDATE_BLUEPRINT_PLACEMENT, {
			handle,
			anchor,
			mode: 'new_base',
			target_guild: targetGuild
		});
	}

	async place(
		handle: string,
		anchor: PlacementAnchor,
		targetGuild: string,
		targetPlayer: string,
		overrideWarnings: boolean
	): Promise<PlaceBlueprintResponse> {
		return sendAndWait<PlaceBlueprintResponse>(MessageType.PLACE_BLUEPRINT, {
			handle,
			anchor,
			mode: 'new_base',
			target_guild: targetGuild,
			target_player: targetPlayer,
			override_warnings: overrideWarnings
		});
	}

	reset(): void {
		this.rows = [];
		this.current = null;
	}
}

export const blueprintsData = new Blueprints();
