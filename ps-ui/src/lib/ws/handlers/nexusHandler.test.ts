import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MessageType } from '$types';
import { ModsState } from '$lib/states/modsState.svelte';
import { NexusState } from '$lib/states/nexusState.svelte';

const holder = vi.hoisted(() => ({ mods: null as any, nexus: null as any }));
const toast = vi.hoisted(() => ({ add: vi.fn() }));

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('$states', () => ({
	getModsState: () => holder.mods,
	getNexusState: () => holder.nexus,
	getToastState: () => toast,
	getModalState: () => ({ showModal: vi.fn() })
}));

const { send } = await import('$lib/utils/websocketUtils');
const { nexusHandlers } = await import('./nexusHandler');

function handlerFor(type: MessageType) {
	const found = nexusHandlers.find((entry) => entry.type === type);
	if (!found) throw new Error(`no handler for ${type}`);
	return found;
}

const context = {} as never;

describe('nexus handlers', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		holder.mods = new ModsState();
		holder.nexus = new NexusState();
		toast.add.mockReset();
	});

	it('renders a nexus_search refusal inline only, never as a toast', async () => {
		holder.nexus.search();
		await handlerFor(MessageType.NEXUS_SEARCH).handle(
			{ offset: 0, error: { code: 'network', message: 'no route' } },
			context
		);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_SEARCH)?.code).toBe('network');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('still toasts a nexus_categories refusal, which has no inline renderer', async () => {
		holder.nexus.loadCategories();
		await handlerFor(MessageType.NEXUS_CATEGORIES).handle(
			{ error: { code: 'network', message: 'no route' } },
			context
		);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_CATEGORIES)?.code).toBe('network');
		expect(toast.add).toHaveBeenCalledTimes(1);
	});

	it('stores the account and clears the busy flag', async () => {
		holder.nexus.loadAccount();
		await handlerFor(MessageType.NEXUS_ACCOUNT_GET).handle(
			{
				signed_in: true,
				account: { user_id: 7, name: 'Tester', is_premium: true, is_supporter: false, profile_url: null },
				rate_limit: null
			},
			context
		);
		expect(holder.nexus.account?.name).toBe('Tester');
		expect(holder.nexus.signedIn).toBe(true);
		expect(holder.nexus.isBusy('account')).toBe(false);
	});

	it('records a refused sign-in and leaves the account alone', async () => {
		holder.nexus.signIn();
		expect(holder.nexus.isBusy('signIn')).toBe(true);
		await handlerFor(MessageType.NEXUS_SIGN_IN).handle(
			{ error: { code: 'sign_in_refused', message: 'Nexus Mods refused the sign-in' } },
			context
		);
		expect(holder.nexus.signedIn).toBe(false);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_SIGN_IN)?.code).toBe('sign_in_refused');
		expect(holder.nexus.isBusy('signIn')).toBe(false);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('signs the account out when Nexus Mods reports the sign-in expired', async () => {
		holder.nexus.applyAccount({
			signed_in: true,
			account: { user_id: 7, name: 'Tester', is_premium: true, is_supporter: false, profile_url: null },
			rate_limit: null
		});
		holder.nexus.loadAccount();
		await handlerFor(MessageType.NEXUS_ACCOUNT_GET).handle(
			{ error: { code: 'sign_in_expired', message: 'expired' } },
			context
		);
		expect(holder.nexus.signedIn).toBe(false);
		expect(holder.nexus.account).toBeNull();
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_ACCOUNT_GET)?.code).toBe('sign_in_expired');
	});

	it('signs the account out when any authenticated reply reports the sign-in gone', async () => {
		const signedIn = {
			signed_in: true,
			account: { user_id: 7, name: 'Tester', is_premium: true, is_supporter: false, profile_url: null },
			rate_limit: null
		};
		holder.nexus.applyAccount(signedIn);
		holder.nexus.loadCategories();
		await handlerFor(MessageType.NEXUS_CATEGORIES).handle(
			{ error: { code: 'sign_in_expired', message: 'expired' } },
			context
		);
		expect(holder.nexus.signedIn).toBe(false);

		holder.nexus.applyAccount(signedIn);
		holder.nexus.startDownload({ target_id: 't1', mod_id: 4821, file_id: 99001 });
		await handlerFor(MessageType.NEXUS_DOWNLOAD).handle(
			{
				target_id: 't1',
				nexus_mod_id: 4821,
				file_id: 99001,
				error: { code: 'sign_in_required', message: 'sign in first' }
			},
			context
		);
		expect(holder.nexus.signedIn).toBe(false);
		expect(holder.nexus.account).toBeNull();

		holder.nexus.applyAccount(signedIn);
		holder.nexus.loadCategories();
		await handlerFor(MessageType.NEXUS_CATEGORIES).handle(
			{ error: { code: 'network', message: 'no route' } },
			context
		);
		expect(holder.nexus.signedIn).toBe(true);
	});

	it('stays signed in when the sign-in went through but the account could not be read', async () => {
		holder.nexus.signIn();
		await handlerFor(MessageType.NEXUS_SIGN_IN).handle(
			{ signed_in: true, error: { code: 'network', message: 'no route' } },
			context
		);
		expect(holder.nexus.signedIn).toBe(true);
		expect(holder.nexus.account).toBeNull();
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_SIGN_IN)?.code).toBe('network');

		holder.nexus.loadAccount();
		await handlerFor(MessageType.NEXUS_ACCOUNT_GET).handle(
			{
				signed_in: true,
				account: { user_id: 7, name: 'Tester', is_premium: false, is_supporter: false, profile_url: null },
				rate_limit: null
			},
			context
		);
		expect(holder.nexus.account?.name).toBe('Tester');
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_SIGN_IN)).toBeUndefined();
	});

	it('treats a cancelled sign-in as no error at all', async () => {
		holder.nexus.signIn();
		await handlerFor(MessageType.NEXUS_SIGN_IN).handle(
			{ error: { code: 'sign_in_cancelled', message: 'cancelled' } },
			context
		);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_SIGN_IN)).toBeUndefined();
		expect(holder.nexus.isBusy('signIn')).toBe(false);
	});

	it('stores the account a finished sign-in returns', async () => {
		holder.nexus.signIn();
		await handlerFor(MessageType.NEXUS_SIGN_IN).handle(
			{
				signed_in: true,
				account: { user_id: 7, name: 'Tester', is_premium: false, is_supporter: false, profile_url: null },
				rate_limit: null
			},
			context
		);
		expect(holder.nexus.account?.name).toBe('Tester');
		expect(holder.nexus.isBusy('signIn')).toBe(false);
	});

	it('stores a search page', async () => {
		holder.nexus.search();
		await handlerFor(MessageType.NEXUS_SEARCH).handle(
			{ offset: 0, count: 1, total_count: 1, mods: [{ mod_id: 4821, name: 'A', summary: null, version: null, author: null, uploader: null, picture_url: null, thumbnail_url: null, endorsements: 0, downloads: 0, file_size: null, adult_content: false, created_at: null, updated_at: null, category: null }] },
			context
		);
		expect(holder.nexus.results).toHaveLength(1);
		expect(holder.nexus.isBusy('searching')).toBe(false);
	});

	it('releases the per-file download flag on an installed reply and reloads the library', async () => {
		holder.nexus.startDownload({ target_id: 't1', mod_id: 4821, file_id: 99001 });
		await handlerFor(MessageType.NEXUS_DOWNLOAD).handle(
			{
				target_id: 't1',
				nexus_mod_id: 4821,
				file_id: 99001,
				version: '1.2.0',
				file_name: 'mod.zip',
				mod_id: 'nexus-4821',
				version_id: 'nexus-4821@1.2.0',
				manifest: {},
				enable_error: null
			},
			context
		);
		expect(holder.nexus.downloadingFile(4821, 99001)).toBe(false);
		expect((send as ReturnType<typeof vi.fn>).mock.calls.some((call) => call[0] === MessageType.MOD_LIST)).toBe(true);
	});

	it('releases the flag on a refusal and records it against the file', async () => {
		holder.nexus.startDownload({ target_id: 't1', mod_id: 4821, file_id: 99001 });
		await handlerFor(MessageType.NEXUS_DOWNLOAD).handle(
			{
				target_id: 't1',
				nexus_mod_id: 4821,
				file_id: 99001,
				error: { code: 'premium_required', message: 'downloading directly needs Premium' }
			},
			context
		);
		expect(holder.nexus.downloadingFile(4821, 99001)).toBe(false);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_DOWNLOAD, 't1')?.code).toBe('premium_required');
	});

	it('keeps a needs_decisions reply as pending, not as a refusal', async () => {
		holder.nexus.startDownload({ target_id: 't1', mod_id: 4821, file_id: 99001 });
		await handlerFor(MessageType.NEXUS_DOWNLOAD).handle(
			{
				target_id: 't1',
				nexus_mod_id: 4821,
				file_id: 99001,
				version: '1.2.0',
				file_name: 'mod.zip',
				needs_decisions: [{ kind: 'nexus_variant', existing_mod_id: 'nexus-4821', file_name: 'mod.zip' }]
			},
			context
		);
		expect(holder.nexus.downloadingFile(4821, 99001)).toBe(false);
		expect(holder.nexus.pendingDecisions?.file_id).toBe(99001);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_DOWNLOAD, 't1')).toBeUndefined();
	});

	it('stores update results and an ignore', async () => {
		holder.nexus.checkUpdates('t1');
		await handlerFor(MessageType.MOD_UPDATE_CHECK).handle(
			{
				target_id: 't1',
				checked: 1,
				truncated: false,
				updates: [{ mod_id: 'nexus-1', nexus_mod_id: 1, installed_version: '1.0', latest: null, state: 'available', ignored_version: null }]
			},
			context
		);
		expect(holder.nexus.updates['nexus-1'].state).toBe('available');

		await handlerFor(MessageType.MOD_UPDATE_IGNORE).handle(
			{ mod_id: 'nexus-1', ignored_version: '2.0' },
			context
		);
		expect(holder.nexus.updates['nexus-1'].ignored_version).toBe('2.0');
	});

	it('forgets a refused update check so a later check retries', async () => {
		holder.nexus.checkUpdates('t1');
		await handlerFor(MessageType.MOD_UPDATE_CHECK).handle(
			{
				target_id: 't1',
				error: { code: 'rate_limited', message: 'slow down', reset: '2026-09-18T00:00:00+00:00' }
			},
			context
		);
		expect(holder.nexus.checkedUpdates('t1')).toBe(false);
		expect(holder.nexus.isBusy('checkingUpdates')).toBe(false);
		expect(holder.mods.lastErrorFor(MessageType.MOD_UPDATE_CHECK, 't1')?.code).toBe('rate_limited');
	});

	it('queues a pushed link without treating its error as a refusal', async () => {
		await handlerFor(MessageType.NEXUS_LINK).handle(
			{ error: { code: 'unsupported_link', message: 'that link is for another game' } },
			context
		);
		expect(holder.nexus.links).toHaveLength(1);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_LINK)).toBeUndefined();
	});

	it('stores the handler status and the foreign-handler refusal', async () => {
		holder.nexus.loadHandler();
		await handlerFor(MessageType.NEXUS_HANDLER_STATUS).handle(
			{ supported: true, registered: false, foreign: true, current: '"C:\\\\Vortex\\\\Vortex.exe" -d "%1"' },
			context
		);
		expect(holder.nexus.handler?.foreign).toBe(true);

		holder.nexus.registerHandler();
		await handlerFor(MessageType.NEXUS_HANDLER_REGISTER).handle(
			{ error: { code: 'foreign_handler', message: 'another program handles nxm links', current: 'Vortex' } },
			context
		);
		expect(holder.mods.lastErrorFor(MessageType.NEXUS_HANDLER_REGISTER)?.code).toBe('foreign_handler');
		expect(holder.nexus.isBusy('handler')).toBe(false);
	});
});
