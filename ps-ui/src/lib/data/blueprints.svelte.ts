import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';
import type {
	BlueprintRow,
	CaptureBlueprintResponse,
	CaptureOptions,
	BlueprintFinding,
	BlueprintGeometry,
	PlacementAnchor,
	ValidatePlacementResponse,
	PlaceBlueprintResponse
} from '$types';

export type BlueprintFormat = 'psbp' | 'json';

// A refused load_blueprint answers under its own message type with `error` (and, for a
// failed reconciliation, `findings`) rather than the dispatcher's hard `error` frame, so
// the request/response waiter still resolves. `handle`/`header` are then absent.
type LoadBlueprintResponse = Partial<CaptureBlueprintResponse> & { error?: string };

class Blueprints {
	rows: BlueprintRow[] = $state([]);
	current: CaptureBlueprintResponse | null = $state(null);
	lastImportFindings: BlueprintFinding[] = $state([]);

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
		const res = await sendAndWait<LoadBlueprintResponse>(MessageType.LOAD_BLUEPRINT, { id });
		return this.acceptLoadResponse(res);
	}

	async loadFromContent(
		content: string,
		format: BlueprintFormat,
		filename?: string
	): Promise<CaptureBlueprintResponse> {
		const res = await sendAndWait<LoadBlueprintResponse>(MessageType.LOAD_BLUEPRINT, {
			content,
			format,
			filename
		});
		return this.acceptLoadResponse(res);
	}

	// A refused load carries its findings before the error is thrown, so the findings
	// panel can still explain why (e.g. a failed reconciliation), even though the caller's
	// catch block is what puts the error itself in front of the user.
	private acceptLoadResponse(res: LoadBlueprintResponse): CaptureBlueprintResponse {
		this.lastImportFindings = res.findings ?? [];
		if (res.error || !res.handle || !res.header) {
			throw new Error(res.error ?? 'Blueprint failed to load');
		}
		const loaded = res as CaptureBlueprintResponse;
		this.current = loaded;
		return loaded;
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
		this.lastImportFindings = [];
	}
}

export const blueprintsData = new Blueprints();
