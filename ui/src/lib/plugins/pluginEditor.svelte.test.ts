import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const sendAndWait = vi.fn();
const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: (type: unknown, data?: unknown) => send(type, data)
}));

import { MANIFEST_PATH, pluginEditor } from './pluginEditor.svelte';

const manifest = {
	id: 'user.one',
	api_version: 1,
	name: 'User One',
	version: '0.1.0',
	entry: 'main.lua',
	capabilities: ['log'],
	commands: [{ id: 'run', title: 'Run', description: null, destructive: false, params: [] }]
};

function openResponse(overrides: Record<string, unknown> = {}) {
	return {
		id: 'user.one',
		manifest,
		sources: { 'main.lua': 'function run()\nend\n' },
		enabled: true,
		bundled: false,
		granted_capabilities: ['log'],
		...overrides
	};
}

beforeEach(() => {
	sendAndWait.mockReset();
	send.mockReset();
	pluginEditor.reset();
});

describe('open', () => {
	it('requests the plugin and lays out its files with the manifest first', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());

		await pluginEditor.open('user.one');

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.GET_PLUGIN, { id: 'user.one' });
		expect(pluginEditor.pluginId).toBe('user.one');
		expect(pluginEditor.paths).toEqual([MANIFEST_PATH, 'main.lua']);
		expect(pluginEditor.activePath).toBe('main.lua');
	});

	it('renders the manifest as indented json', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
		expect(pluginEditor.files[MANIFEST_PATH]).toBe(JSON.stringify(manifest, null, 2));
	});

	it('records the grant, the bundled flag and the enabled flag', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse({ bundled: true, enabled: false }));
		await pluginEditor.open('user.one');
		expect(pluginEditor.granted).toEqual(['log']);
		expect(pluginEditor.bundled).toBe(true);
		expect(pluginEditor.enabled).toBe(false);
	});

	it('starts with nothing dirty', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
		expect(pluginEditor.dirty).toBe(false);
	});

	it('discards a slow open() if reset() ran before it resolved', async () => {
		let resolveOpen: (value: unknown) => void;
		sendAndWait.mockReturnValueOnce(
			new Promise((resolve) => {
				resolveOpen = resolve;
			})
		);

		const openPromise = pluginEditor.open('user.one');
		pluginEditor.reset();
		resolveOpen!(openResponse());
		await openPromise;

		expect(pluginEditor.pluginId).toBeNull();
		expect(pluginEditor.files).toEqual({});
		expect(pluginEditor.loading).toBe(false);
	});

	it('discards a slow open("a") if open("b") started after it', async () => {
		let resolveA: (value: unknown) => void;
		let resolveB: (value: unknown) => void;
		sendAndWait.mockReturnValueOnce(
			new Promise((resolve) => {
				resolveA = resolve;
			})
		);

		const openA = pluginEditor.open('a');

		sendAndWait.mockReturnValueOnce(
			new Promise((resolve) => {
				resolveB = resolve;
			})
		);
		const openB = pluginEditor.open('b');

		resolveA!(openResponse({ id: 'a' }));
		await openA;
		resolveB!(openResponse({ id: 'b' }));
		await openB;

		expect(pluginEditor.pluginId).toBe('b');
		expect(pluginEditor.loading).toBe(false);
	});
});

describe('setSource', () => {
	beforeEach(async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
	});

	it('marks only the edited file dirty', () => {
		pluginEditor.setSource('main.lua', 'function run() return 1 end\n');
		expect(pluginEditor.isDirty('main.lua')).toBe(true);
		expect(pluginEditor.isDirty(MANIFEST_PATH)).toBe(false);
		expect(pluginEditor.dirty).toBe(true);
	});

	it('is clean again when the text is restored', () => {
		const original = pluginEditor.files['main.lua'];
		pluginEditor.setSource('main.lua', 'changed');
		pluginEditor.setSource('main.lua', original);
		expect(pluginEditor.isDirty('main.lua')).toBe(false);
	});
});

describe('checkActive', () => {
	beforeEach(async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
	});

	it('syntax-checks a lua file and stores the error', async () => {
		sendAndWait.mockResolvedValueOnce({ error: { line: 2, message: 'unexpected symbol' } });
		pluginEditor.activePath = 'main.lua';
		pluginEditor.setSource('main.lua', 'local a = 1\nlocal b = = 2\n');

		await pluginEditor.checkActive();

		expect(sendAndWait).toHaveBeenLastCalledWith(MessageType.CHECK_PLUGIN_SYNTAX, {
			source: 'local a = 1\nlocal b = = 2\n'
		});
		expect(pluginEditor.syntaxError).toEqual({ line: 2, message: 'unexpected symbol' });
	});

	it('clears a previous syntax error when the source parses', async () => {
		sendAndWait.mockResolvedValueOnce({ error: { line: 1, message: 'bad' } });
		pluginEditor.activePath = 'main.lua';
		await pluginEditor.checkActive();
		sendAndWait.mockResolvedValueOnce({ error: null });
		await pluginEditor.checkActive();
		expect(pluginEditor.syntaxError).toBeNull();
	});

	it('validates the manifest under the plugins own id', async () => {
		sendAndWait.mockResolvedValueOnce({ error: 'unsupported api version' });
		pluginEditor.activePath = MANIFEST_PATH;

		await pluginEditor.checkActive();

		expect(sendAndWait).toHaveBeenLastCalledWith(MessageType.CHECK_PLUGIN_MANIFEST, {
			id: 'user.one',
			manifest: pluginEditor.files[MANIFEST_PATH]
		});
		expect(pluginEditor.manifestError).toBe('unsupported api version');
	});

	it('sends an empty manifest rather than omitting the field', async () => {
		sendAndWait.mockResolvedValueOnce({ error: null });
		pluginEditor.activePath = MANIFEST_PATH;
		pluginEditor.files = {};

		await pluginEditor.checkActive();

		expect(sendAndWait).toHaveBeenLastCalledWith(MessageType.CHECK_PLUGIN_MANIFEST, {
			id: 'user.one',
			manifest: ''
		});
	});

	it('never syntax-checks the manifest as lua', async () => {
		sendAndWait.mockResolvedValueOnce({ error: null });
		pluginEditor.activePath = MANIFEST_PATH;
		await pluginEditor.checkActive();
		expect(sendAndWait).not.toHaveBeenCalledWith(
			MessageType.CHECK_PLUGIN_SYNTAX,
			expect.anything()
		);
	});
});

describe('saveActive', () => {
	beforeEach(async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
	});

	it('sends the active path and its draft text, then marks it clean', async () => {
		pluginEditor.activePath = 'main.lua';
		pluginEditor.setSource('main.lua', 'function run() return 1 end\n');
		sendAndWait.mockResolvedValueOnce({ id: 'user.one', path: 'main.lua' });

		await pluginEditor.saveActive();

		expect(sendAndWait).toHaveBeenLastCalledWith(MessageType.SAVE_PLUGIN_SOURCE, {
			id: 'user.one',
			path: 'main.lua',
			source: 'function run() return 1 end\n'
		});
		expect(pluginEditor.isDirty('main.lua')).toBe(false);
	});

	it('leaves the file dirty when the save is refused', async () => {
		pluginEditor.activePath = 'main.lua';
		pluginEditor.setSource('main.lua', 'changed');
		sendAndWait.mockRejectedValueOnce(new Error('plugin is bundled'));

		await expect(pluginEditor.saveActive()).rejects.toThrow('bundled');
		expect(pluginEditor.isDirty('main.lua')).toBe(true);
	});

	it('throws the refusal the response carries, and keeps the file dirty', async () => {
		pluginEditor.activePath = MANIFEST_PATH;
		pluginEditor.setSource(MANIFEST_PATH, '{ "id": "other.thing" }');
		sendAndWait.mockResolvedValueOnce({
			id: 'user.one',
			path: MANIFEST_PATH,
			error: 'this manifest declares id "other.thing"'
		});

		await expect(pluginEditor.saveActive()).rejects.toThrow('other.thing');
		expect(pluginEditor.isDirty(MANIFEST_PATH)).toBe(true);
		expect(pluginEditor.saving).toBe(false);
	});
});

describe('commandIds and warnings', () => {
	beforeEach(async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
	});

	it('reads the command ids out of the draft manifest', () => {
		expect(pluginEditor.commandIds).toEqual(['run']);
	});

	it('reads them from the edited text, not the saved text', () => {
		pluginEditor.setSource(
			MANIFEST_PATH,
			JSON.stringify({ ...manifest, commands: [{ id: 'clean', title: 'Clean' }] }, null, 2)
		);
		expect(pluginEditor.commandIds).toEqual(['clean']);
	});

	it('yields no command ids for a manifest that is not valid json', () => {
		pluginEditor.setSource(MANIFEST_PATH, '{ broken');
		expect(pluginEditor.commandIds).toEqual([]);
	});

	it('warns when a declared command has no function in the entry source', () => {
		pluginEditor.setSource(
			MANIFEST_PATH,
			JSON.stringify({ ...manifest, commands: [{ id: 'clean', title: 'Clean' }] }, null, 2)
		);
		const kinds = pluginEditor.warnings.map((w) => w.kind);
		expect(kinds).toContain('command-without-function');
		expect(kinds).toContain('function-without-command');
	});

	it('is silent when the manifest and the entry source agree', () => {
		expect(pluginEditor.warnings).toEqual([]);
	});
});

describe('runDraft', () => {
	it('sends the draft sources and the draft manifest', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
		pluginEditor.setSource('main.lua', 'function run() return 1 end\n');

		pluginEditor.runDraft('run', {}, true);

		expect(send).toHaveBeenCalledWith(
			MessageType.RUN_PLUGIN_DRAFT,
			expect.objectContaining({
				plugin_id: 'user.one',
				command_id: 'run',
				dry_run: true,
				sources: { 'main.lua': 'function run() return 1 end\n' }
			})
		);
	});

	it('sends nothing for a bundled plugin', async () => {
		sendAndWait.mockResolvedValueOnce(openResponse({ bundled: true }));
		await pluginEditor.open('user.one');

		pluginEditor.runDraft('run', {}, false);

		expect(send).not.toHaveBeenCalled();
	});
});

describe('the destructive draft rail', () => {
	const destructiveManifest = JSON.stringify(
		{ ...manifest, commands: [{ id: 'run', title: 'Run', destructive: true }] },
		null,
		2
	);

	beforeEach(async () => {
		sendAndWait.mockResolvedValueOnce(openResponse());
		await pluginEditor.open('user.one');
		pluginEditor.setSource(MANIFEST_PATH, destructiveManifest);
	});

	it('forces the dry run even when the checkbox is off', () => {
		pluginEditor.runDraft('run', {}, false);

		expect(send).toHaveBeenCalledWith(
			MessageType.RUN_PLUGIN_DRAFT,
			expect.objectContaining({ dry_run: true })
		);
		expect(pluginEditor.pendingApply).not.toBeNull();
	});

	it('applies only on the second, explicit action', () => {
		pluginEditor.runDraft('run', {}, false);
		send.mockClear();

		pluginEditor.applyPending();

		expect(send).toHaveBeenCalledWith(
			MessageType.RUN_PLUGIN_DRAFT,
			expect.objectContaining({ dry_run: false })
		);
		expect(pluginEditor.pendingApply).toBeNull();
	});

	it('applies the previewed draft, not whatever the buffer holds by then', () => {
		pluginEditor.runDraft('run', {}, false);
		pluginEditor.setSource('main.lua', 'function run() return "edited after the preview" end\n');
		send.mockClear();

		pluginEditor.applyPending();

		expect(send).toHaveBeenCalledWith(
			MessageType.RUN_PLUGIN_DRAFT,
			expect.objectContaining({ sources: { 'main.lua': 'function run()\nend\n' } })
		);
	});

	it('sends nothing when the pending apply is cancelled', () => {
		pluginEditor.runDraft('run', {}, false);
		send.mockClear();

		pluginEditor.cancelPending();
		pluginEditor.applyPending();

		expect(send).not.toHaveBeenCalled();
		expect(pluginEditor.pendingApply).toBeNull();
	});

	it('leaves a non-destructive command free to run for real', () => {
		pluginEditor.setSource(MANIFEST_PATH, JSON.stringify(manifest, null, 2));

		pluginEditor.runDraft('run', {}, false);

		expect(send).toHaveBeenCalledWith(
			MessageType.RUN_PLUGIN_DRAFT,
			expect.objectContaining({ dry_run: false })
		);
		expect(pluginEditor.pendingApply).toBeNull();
	});
});

describe('loadDefinition', () => {
	it('fetches the definition once and reuses it', async () => {
		sendAndWait.mockResolvedValue({ globals: [], handles: [] });

		await pluginEditor.loadDefinition();
		await pluginEditor.loadDefinition();

		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(sendAndWait).toHaveBeenCalledWith(MessageType.GET_API_DEFINITION, undefined);
		expect(pluginEditor.definition).toEqual({ globals: [], handles: [] });
	});
});

describe('create', () => {
	it('sends the id and name and returns the created id', async () => {
		sendAndWait.mockResolvedValueOnce({ id: 'my.first', name: 'My First' });

		const id = await pluginEditor.create('my.first', 'My First');

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.CREATE_PLUGIN, {
			id: 'my.first',
			name: 'My First'
		});
		expect(id).toBe('my.first');
	});

	it('throws the refusal the response carries', async () => {
		sendAndWait.mockResolvedValueOnce({ error: 'plugin "my.first" already exists' });

		await expect(pluginEditor.create('my.first', 'My First')).rejects.toThrow('already exists');
	});
});
