import { send } from '$lib/utils/websocketUtils';
import type {
	ModUpdate,
	ModUpdateCheckReply,
	ModUpdateIgnoreReply,
	NexusAccount,
	NexusAccountReply,
	NexusCategoriesReply,
	NexusCategory,
	NexusDownloadReply,
	NexusDownloadRequest,
	NexusFilesReply,
	NexusHandlerStatus,
	NexusLinkPush,
	NexusModSummary,
	NexusRateLimit,
	NexusSearchParams,
	NexusSearchReply,
	NexusSort
} from '$types';
import { MessageType } from '$types';

export type NexusBusyKind =
	| 'account'
	| 'categories'
	| 'searching'
	| 'files'
	| 'checkingUpdates'
	| 'ignoring'
	| 'handler';

export const NEXUS_PAGE_SIZE = 20;
const MAX_OFFSET = 10_000;

export function downloadKey(modId: number, fileId: number): string {
	return `${modId}:${fileId}`;
}

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
	const { [key]: _removed, ...rest } = record;
	return rest;
}

export class NexusState {
	hasKey = $state(false);
	account = $state<NexusAccount | null>(null);
	rateLimit = $state<NexusRateLimit | null>(null);
	categories = $state<NexusCategory[]>([]);

	query = $state('');
	category = $state<string | null>(null);
	sort = $state<NexusSort | null>(null);
	includeAdult = $state(false);

	results = $state<NexusModSummary[]>([]);
	totalCount = $state(0);
	searched = $state(false);

	files = $state<Record<number, NexusFilesReply>>({});
	updates = $state<Record<string, ModUpdate>>({});
	updateRuns = $state<Record<string, { checked: number; truncated: boolean }>>({});
	links = $state<NexusLinkPush[]>([]);
	handler = $state<NexusHandlerStatus | null>(null);
	pendingDecisions = $state<NexusDownloadReply | null>(null);
	lastInstalled = $state<Record<string, NexusDownloadReply>>({});

	#busy = $state<Record<string, number>>({});
	#downloading = $state<Record<string, string>>({});
	/** Not a real `#` field: private fields can't be `$state` in a class. Never read this from a component. */
	nextOffset = $state(0);
	#linkSubscribed = false;
	#lastResets: number | undefined;

	get hasMore(): boolean {
		return this.nextOffset < this.totalCount && this.nextOffset < MAX_OFFSET;
	}

	/** Lives here, not in the route, so a `resets` bump that lands while /mods is unmounted is
	 *  still noticed on the next mount instead of being silently missed. */
	shouldForget(resets: number): boolean {
		const changed = this.#lastResets !== undefined && resets !== this.#lastResets;
		this.#lastResets = resets;
		return changed;
	}

	isBusy(kind: NexusBusyKind): boolean {
		return (this.#busy[kind] ?? 0) > 0;
	}

	downloadingFile(modId: number, fileId: number): boolean {
		return this.#downloading[downloadKey(modId, fileId)] !== undefined;
	}

	downloadTarget(modId: number, fileId: number): string | undefined {
		return this.#downloading[downloadKey(modId, fileId)];
	}

	checkedUpdates(targetId: string): boolean {
		return this.updateRuns[targetId] !== undefined;
	}

	updateFor(modId: string): ModUpdate | undefined {
		return this.updates[modId];
	}

	#startBusy(kind: NexusBusyKind): void {
		this.#busy = { ...this.#busy, [kind]: (this.#busy[kind] ?? 0) + 1 };
	}

	finishBusy(kind: NexusBusyKind): void {
		const remaining = Math.max(0, (this.#busy[kind] ?? 0) - 1);
		this.#busy = { ...this.#busy, [kind]: remaining };
	}

	loadAccount(): void {
		this.#startBusy('account');
		send(MessageType.NEXUS_ACCOUNT_GET, {});
	}

	/** The key is passed straight through and never stored here. */
	setKey(key: string): void {
		this.#startBusy('account');
		send(MessageType.NEXUS_KEY_SET, { key });
	}

	clearKey(): void {
		this.#startBusy('account');
		send(MessageType.NEXUS_KEY_CLEAR, {});
	}

	loadCategories(): void {
		this.#startBusy('categories');
		send(MessageType.NEXUS_CATEGORIES, {});
	}

	#searchParams(offset: number): NexusSearchParams {
		const query = this.query.trim();
		const params: NexusSearchParams = {
			offset,
			count: NEXUS_PAGE_SIZE,
			include_adult: this.includeAdult
		};
		if (query) params.query = query;
		if (this.category) params.category = this.category;
		params.sort = this.sort ?? (query ? 'relevance' : 'downloads');
		return params;
	}

	search(): void {
		this.nextOffset = 0;
		this.#startBusy('searching');
		send(MessageType.NEXUS_SEARCH, this.#searchParams(0));
	}

	loadMore(): void {
		if (!this.hasMore || this.isBusy('searching')) return;
		this.#startBusy('searching');
		send(MessageType.NEXUS_SEARCH, this.#searchParams(this.nextOffset));
	}

	loadFiles(modId: number): void {
		this.#startBusy('files');
		send(MessageType.NEXUS_MOD_FILES, { mod_id: modId });
	}

	/** False when this mod and file are already downloading: the server takes no lock. */
	startDownload(request: NexusDownloadRequest): boolean {
		const key = downloadKey(request.mod_id, request.file_id);
		if (this.#downloading[key] !== undefined) return false;
		this.#downloading = { ...this.#downloading, [key]: request.target_id };
		send(MessageType.NEXUS_DOWNLOAD, request);
		return true;
	}

	finishDownload(modId: number, fileId: number): void {
		this.#downloading = without(this.#downloading, downloadKey(modId, fileId));
	}

	checkUpdates(targetId: string, options: { force?: boolean } = {}): void {
		if (!options.force && this.checkedUpdates(targetId)) return;
		if (this.isBusy('checkingUpdates')) return;
		this.updateRuns = { ...this.updateRuns, [targetId]: { checked: 0, truncated: false } };
		this.#startBusy('checkingUpdates');
		send(MessageType.MOD_UPDATE_CHECK, { target_id: targetId });
	}

	ignoreVersion(modId: string, version: string | null): void {
		this.#startBusy('ignoring');
		send(MessageType.MOD_UPDATE_IGNORE, { mod_id: modId, version });
	}

	loadHandler(): void {
		this.#startBusy('handler');
		send(MessageType.NEXUS_HANDLER_STATUS, {});
	}

	registerHandler(force = false): void {
		this.#startBusy('handler');
		send(MessageType.NEXUS_HANDLER_REGISTER, { force });
	}

	subscribeLinks(): void {
		if (this.#linkSubscribed) return;
		this.#linkSubscribed = true;
		send(MessageType.NEXUS_LINK_SUBSCRIBE, {});
	}

	applyAccount(reply: NexusAccountReply): void {
		this.hasKey = reply.has_key;
		this.account = reply.account;
		this.rateLimit = reply.rate_limit;
	}

	applyCategories(reply: NexusCategoriesReply): void {
		this.categories = reply.categories;
	}

	/**
	 * A page that answers neither the first request nor the awaited offset is stale. A first page
	 * is dropped too while another search is still outstanding, so only the last one to settle wins.
	 */
	applySearch(reply: NexusSearchReply): void {
		if (reply.offset !== 0 && reply.offset !== this.nextOffset) return;
		if (reply.offset === 0 && this.isBusy('searching')) return;
		this.results = reply.offset === 0 ? reply.mods : [...this.results, ...reply.mods];
		this.totalCount = reply.total_count;
		this.nextOffset = reply.offset + reply.count;
		this.searched = true;
	}

	applyFiles(reply: NexusFilesReply): void {
		this.files = { ...this.files, [reply.mod_id]: reply };
	}

	applyUpdates(reply: ModUpdateCheckReply): void {
		const merged = { ...this.updates };
		for (const update of reply.updates) merged[update.mod_id] = update;
		this.updates = merged;
		if (reply.target_id) {
			this.updateRuns = {
				...this.updateRuns,
				[reply.target_id]: { checked: reply.checked, truncated: reply.truncated }
			};
		}
	}

	/** A known latest lets a mismatched ignore leave the row's state alone; an unknown one trusts the ignore. */
	applyIgnore(reply: ModUpdateIgnoreReply): void {
		const stored = this.updates[reply.mod_id];
		if (!stored) return;
		const ignored = reply.ignored_version;
		const latest = stored.latest?.version ?? null;
		const state: ModUpdate['state'] = ignored
			? latest === null || ignored === latest
				? 'ignored'
				: stored.state
			: stored.state === 'ignored'
				? 'available'
				: stored.state;
		this.updates = {
			...this.updates,
			[reply.mod_id]: { ...stored, ignored_version: ignored, state }
		};
	}

	applyHandler(status: NexusHandlerStatus): void {
		this.handler = status;
	}

	holdDecisions(reply: NexusDownloadReply): void {
		this.pendingDecisions = reply;
	}

	clearDecisions(): void {
		this.pendingDecisions = null;
	}

	recordInstalled(reply: NexusDownloadReply): void {
		this.lastInstalled = {
			...this.lastInstalled,
			[downloadKey(reply.nexus_mod_id, reply.file_id)]: reply
		};
		const modId = reply.mod_id;
		if (!modId) return;
		const stored = this.updates[modId];
		if (stored && stored.latest?.file_id === reply.file_id) {
			this.updates = {
				...this.updates,
				[modId]: {
					...stored,
					installed_version: reply.version ?? stored.installed_version,
					state: 'up_to_date'
				}
			};
		}
	}

	/** A refused check must not count as "already checked", or the tab never retries. */
	forgetUpdateRun(targetId: string | undefined): void {
		if (!targetId) return;
		this.updateRuns = without(this.updateRuns, targetId);
	}

	pushLink(payload: NexusLinkPush): void {
		this.links = [...this.links, payload];
	}

	dismissLink(index: number): void {
		this.links = this.links.filter((_, position) => position !== index);
	}

	/** A request sent before a drop is never answered, and the subscription is gone with it. */
	forgetInFlight(): void {
		this.#busy = {};
		this.#downloading = {};
		this.updateRuns = {};
		this.#linkSubscribed = false;
		this.pendingDecisions = null;
	}

	reset(): void {
		this.forgetInFlight();
		this.hasKey = false;
		this.account = null;
		this.rateLimit = null;
		this.categories = [];
		this.results = [];
		this.totalCount = 0;
		this.searched = false;
		this.query = '';
		this.category = null;
		this.sort = null;
		this.includeAdult = false;
		this.files = {};
		this.updates = {};
		this.links = [];
		this.handler = null;
		this.pendingDecisions = null;
		this.lastInstalled = {};
		this.nextOffset = 0;
	}
}

let nexusStateInstance: NexusState | undefined;

export function getNexusState(): NexusState {
	nexusStateInstance ??= new NexusState();
	return nexusStateInstance;
}
