import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MessageType } from '$types';

const sendAndWait = vi.fn();
const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: (type: unknown, data?: unknown) => send(type, data)
}));

import { blueprintsData } from './blueprints.svelte';

beforeEach(() => {
	sendAndWait.mockReset();
	send.mockReset();
	blueprintsData.reset();
});

describe('blueprintsData.capture', () => {
	it('sends the base id, options and name, and stores the returned handle+header', async () => {
		const header = { name: 'Home', structure_count: 5 };
		sendAndWait.mockResolvedValueOnce({ handle: 'h1', header });
		const options = { production_config: true } as any;

		const res = await blueprintsData.capture('base-9', options, 'Home');

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.CAPTURE_BASE_BLUEPRINT, {
			base_id: 'base-9',
			options,
			name: 'Home'
		});
		expect(res.handle).toBe('h1');
		expect(blueprintsData.current?.handle).toBe('h1');
		expect(blueprintsData.current?.header.name).toBe('Home');
	});
});

describe('blueprintsData.store', () => {
	it('stores the handle then refreshes the row list from the server', async () => {
		sendAndWait
			.mockResolvedValueOnce({ id: 'row-1' }) // store_blueprint
			.mockResolvedValueOnce({ blueprints: [{ id: 'row-1', name: 'Home' }] }); // list_blueprints

		const id = await blueprintsData.store('h1');

		expect(id).toBe('row-1');
		expect(sendAndWait).toHaveBeenNthCalledWith(1, MessageType.STORE_BLUEPRINT, { handle: 'h1' });
		expect(sendAndWait).toHaveBeenNthCalledWith(2, MessageType.LIST_BLUEPRINTS, undefined);
		expect(blueprintsData.rows.map((r) => r.id)).toEqual(['row-1']);
	});
});

describe('blueprintsData.loadFromContent', () => {
	it('sends the base64 content and format and stores the loaded handle', async () => {
		sendAndWait.mockResolvedValueOnce({ handle: 'h2', header: { name: 'Imported' } });

		const res = await blueprintsData.loadFromContent('QkFTRQ==', 'psp');

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.LOAD_BLUEPRINT, {
			content: 'QkFTRQ==',
			format: 'psp'
		});
		expect(res.header.name).toBe('Imported');
		expect(blueprintsData.current?.handle).toBe('h2');
	});
});

describe('blueprintsData.exportRow', () => {
	it('loads the stored row by id then fires an export for the returned handle', async () => {
		sendAndWait.mockResolvedValueOnce({ handle: 'h3', header: { name: 'Home' } });

		await blueprintsData.exportRow('row-1', 'json');

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.LOAD_BLUEPRINT, { id: 'row-1' });
		expect(send).toHaveBeenCalledWith(MessageType.EXPORT_BLUEPRINT_FILE, {
			handle: 'h3',
			format: 'json'
		});
	});
});
