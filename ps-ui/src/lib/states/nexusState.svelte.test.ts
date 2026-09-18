import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NexusState, downloadKey } from './nexusState.svelte';

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));
const { send } = await import('$lib/utils/websocketUtils');

function sent(type: MessageType) {
	return (send as ReturnType<typeof vi.fn>).mock.calls.filter((call) => call[0] === type);
}

function summary(modId: number) {
	return {
		mod_id: modId,
		name: `Mod ${modId}`,
		summary: null,
		version: null,
		author: null,
		uploader: null,
		picture_url: null,
		thumbnail_url: null,
		endorsements: 0,
		downloads: 0,
		file_size: null,
		adult_content: false,
		created_at: null,
		updated_at: null,
		category: null
	};
}

describe('NexusState', () => {
	let state: NexusState;

	beforeEach(() => {
		vi.clearAllMocks();
		state = new NexusState();
	});

	it('searches from offset zero with the current filters', () => {
		state.query = 'bag';
		state.includeAdult = true;
		state.search();
		expect(sent(MessageType.NEXUS_SEARCH)[0][1]).toEqual({
			query: 'bag',
			sort: 'relevance',
			offset: 0,
			count: 20,
			include_adult: true
		});
		expect(state.isBusy('searching')).toBe(true);
	});

	it('browses by downloads when there is no query, and omits a blank one', () => {
		state.query = '   ';
		state.search();
		const params = sent(MessageType.NEXUS_SEARCH)[0][1];
		expect(params.query).toBeUndefined();
		expect(params.sort).toBe('downloads');
	});

	it('replaces results on a first page and appends the next one', () => {
		state.search();
		state.finishBusy('searching');
		state.applySearch({ offset: 0, count: 2, total_count: 5, mods: [summary(1), summary(2)] });
		expect(state.results.map((mod) => mod.mod_id)).toEqual([1, 2]);
		expect(state.hasMore).toBe(true);

		state.loadMore();
		expect(sent(MessageType.NEXUS_SEARCH)[1][1].offset).toBe(2);
		state.applySearch({ offset: 2, count: 3, total_count: 5, mods: [summary(3)] });
		state.finishBusy('searching');
		expect(state.results.map((mod) => mod.mod_id)).toEqual([1, 2, 3]);
		expect(state.hasMore).toBe(false);
		expect(state.isBusy('searching')).toBe(false);
	});

	it('ignores a page that answers neither the first request nor the next offset', () => {
		state.search();
		state.finishBusy('searching');
		state.applySearch({ offset: 0, count: 1, total_count: 9, mods: [summary(1)] });
		state.applySearch({ offset: 6, count: 1, total_count: 9, mods: [summary(7)] });
		expect(state.results.map((mod) => mod.mod_id)).toEqual([1]);
	});

	it('drops a first-page reply that lands while another search is still outstanding', () => {
		state.search();
		state.query = 'shield';
		state.search();
		expect(state.isBusy('searching')).toBe(true);

		state.finishBusy('searching');
		state.applySearch({ offset: 0, count: 1, total_count: 1, mods: [summary(2)] });
		expect(state.results).toEqual([]);
		expect(state.searched).toBe(false);

		state.finishBusy('searching');
		state.applySearch({ offset: 0, count: 1, total_count: 1, mods: [summary(1)] });
		expect(state.results.map((mod) => mod.mod_id)).toEqual([1]);
		expect(state.searched).toBe(true);
	});

	it('runs one download per mod and file at a time', () => {
		const request = { target_id: 't1', mod_id: 4821, file_id: 99001 };
		expect(state.startDownload(request)).toBe(true);
		expect(state.startDownload(request)).toBe(false);
		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(1);
		expect(state.downloadingFile(4821, 99001)).toBe(true);

		state.finishDownload(4821, 99001);
		expect(state.downloadingFile(4821, 99001)).toBe(false);
		expect(state.startDownload(request)).toBe(true);
	});

	it('sends enable and accept_defaults only when asked', () => {
		state.startDownload({ target_id: 't1', mod_id: 1, file_id: 2, accept_defaults: true });
		expect(sent(MessageType.NEXUS_DOWNLOAD)[0][1]).toEqual({
			target_id: 't1',
			mod_id: 1,
			file_id: 2,
			accept_defaults: true
		});
	});

	it('checks a target once and remembers it, until asked again', () => {
		expect(state.checkedUpdates('t1')).toBe(false);
		state.checkUpdates('t1');
		state.checkUpdates('t1');
		expect(sent(MessageType.MOD_UPDATE_CHECK)).toHaveLength(1);
		state.applyUpdates({
			target_id: 't1',
			checked: 1,
			truncated: false,
			updates: [
				{
					mod_id: 'nexus-4821',
					nexus_mod_id: 4821,
					installed_version: '1.2.0',
					latest: null,
					state: 'available',
					ignored_version: null
				}
			]
		});
		state.finishBusy('checkingUpdates');
		expect(state.checkedUpdates('t1')).toBe(true);
		expect(state.updates['nexus-4821'].state).toBe('available');

		state.checkUpdates('t1', { force: true });
		expect(sent(MessageType.MOD_UPDATE_CHECK)).toHaveLength(2);
	});

	it('keeps an ignored version on the stored update', () => {
		state.applyUpdates({
			target_id: 't1',
			checked: 1,
			truncated: false,
			updates: [
				{
					mod_id: 'nexus-1',
					nexus_mod_id: 1,
					installed_version: '1.0',
					latest: null,
					state: 'available',
					ignored_version: null
				}
			]
		});
		state.applyIgnore({ mod_id: 'nexus-1', ignored_version: '2.0' });
		expect(state.updates['nexus-1'].ignored_version).toBe('2.0');
		expect(state.updates['nexus-1'].state).toBe('ignored');

		state.applyIgnore({ mod_id: 'nexus-1', ignored_version: null });
		expect(state.updates['nexus-1'].state).toBe('available');
	});

	it('subscribes to links once per connection', () => {
		state.subscribeLinks();
		state.subscribeLinks();
		expect(sent(MessageType.NEXUS_LINK_SUBSCRIBE)).toHaveLength(1);

		state.forgetInFlight();
		state.subscribeLinks();
		expect(sent(MessageType.NEXUS_LINK_SUBSCRIBE)).toHaveLength(2);
	});

	it('queues pushed links and dismisses them one at a time', () => {
		state.pushLink({ link: { mod_id: 1, file_id: 2, key: 'k', expires: 99 } });
		state.pushLink({ error: { code: 'unsupported_link', message: 'another game' } });
		expect(state.links).toHaveLength(2);
		state.dismissLink(0);
		expect(state.links).toHaveLength(1);
		expect(state.links[0].error?.code).toBe('unsupported_link');
	});

	it('clears in-flight work on a drop but keeps what the server told it', () => {
		state.applyAccount({ has_key: true, account: null, rate_limit: null });
		state.search();
		state.startDownload({ target_id: 't1', mod_id: 1, file_id: 2 });
		state.checkUpdates('t1');

		state.forgetInFlight();
		expect(state.isBusy('searching')).toBe(false);
		expect(state.downloadingFile(1, 2)).toBe(false);
		expect(state.checkedUpdates('t1')).toBe(false);
		expect(state.hasKey).toBe(true);

		state.reset();
		expect(state.hasKey).toBe(false);
		expect(state.results).toEqual([]);
	});

	it('marks a matching update up to date once its download installs', () => {
		state.applyUpdates({
			target_id: 't1',
			checked: 1,
			truncated: false,
			updates: [
				{
					mod_id: 'nexus-4821',
					nexus_mod_id: 4821,
					installed_version: '1.2.0',
					latest: {
						file_id: 99002,
						name: 'Enhanced Visuals',
						version: '1.3.0',
						category: 'UPDATE',
						date: 0,
						size_in_bytes: null,
						uri: 'file.zip',
						primary: false,
						description: null
					},
					state: 'available',
					ignored_version: null
				}
			]
		});

		state.recordInstalled({
			target_id: 't1',
			nexus_mod_id: 4821,
			file_id: 99002,
			version: '1.3.0',
			file_name: 'file.zip',
			mod_id: 'nexus-4821',
			version_id: 'nexus-4821@1.3.0',
			manifest: undefined
		});

		expect(state.updates['nexus-4821'].state).toBe('up_to_date');
		expect(state.updates['nexus-4821'].installed_version).toBe('1.3.0');
	});

	it('leaves an unrelated update alone when a different file installs', () => {
		state.applyUpdates({
			target_id: 't1',
			checked: 1,
			truncated: false,
			updates: [
				{
					mod_id: 'nexus-4821',
					nexus_mod_id: 4821,
					installed_version: '1.2.0',
					latest: {
						file_id: 99002,
						name: 'Enhanced Visuals',
						version: '1.3.0',
						category: 'UPDATE',
						date: 0,
						size_in_bytes: null,
						uri: 'file.zip',
						primary: false,
						description: null
					},
					state: 'available',
					ignored_version: null
				}
			]
		});

		state.recordInstalled({
			target_id: 't1',
			nexus_mod_id: 4821,
			file_id: 12345,
			version: '1.0.0',
			file_name: 'other.zip',
			mod_id: 'nexus-4821',
			version_id: 'nexus-4821@1.0.0',
			manifest: undefined
		});

		expect(state.updates['nexus-4821'].state).toBe('available');
	});

	it('names a download by mod and file', () => {
		expect(downloadKey(4821, 99001)).toBe('4821:99001');
	});
});
