import { env, evictDurableObject, runDurableObjectAlarm, runInDurableObject } from 'cloudflare:test';
import { expect, it } from 'vitest';
import { ADMISSION, MAX_FRAME_BYTES } from '../src/policy';

const stubFor = (room: string) => env.SIGNAL_ROOM.get(env.SIGNAL_ROOM.idFromName(room));
const open = (room: string, role: 'host' | 'guest', kind?: 'pair' | 'meet') =>
	stubFor(room).fetch(`https://x/signal/ws?room=${room}&role=${role}${kind ? `&kind=${kind}` : ''}`, {
		headers: { Upgrade: 'websocket' },
	});

async function waitForRoleFree(room: string, role: 'host' | 'guest') {
	for (let i = 0; i < 100; i++) {
		const count = await runInDurableObject(stubFor(room), async (_instance, state) => state.getWebSockets(role).length);
		if (count === 0) return;
		await new Promise((r) => setTimeout(r, 5));
	}
	throw new Error(`role ${role} never freed`);
}

it('relays a frame from host to guest and back', async () => {
	const room = '1'.repeat(32);
	const host = (await open(room, 'host')).webSocket!;
	const guest = (await open(room, 'guest')).webSocket!;
	host.accept();
	guest.accept();
	const seen = new Promise<string>((r) => guest.addEventListener('message', (e: MessageEvent) => r(e.data as string)));
	host.send('sealed-blob');
	expect(await seen).toBe('sealed-blob');
});

it('refuses a second host with a 409 the desktop can read', async () => {
	const room = '2'.repeat(32);
	await open(room, 'host');
	expect((await open(room, 'host')).status).toBe(409);
});

it('turns a second guest away with close code 4010 instead of a 409', async () => {
	const room = 'e'.repeat(32);
	await open(room, 'guest');
	const res = await open(room, 'guest');
	expect(res.status).toBe(101);

	const refused = res.webSocket!;
	refused.accept();
	const closed = new Promise<number>((r) => refused.addEventListener('close', (e: CloseEvent) => r(e.code)));
	expect(await closed).toBe(4010);

	await runInDurableObject(stubFor(room), async (_instance, state) => {
		expect(state.getWebSockets('guest').length).toBe(1);
	});
});

it('rejects an oversized frame with close code 4009', async () => {
	const room = '3'.repeat(32);
	const host = (await open(room, 'host')).webSocket!;
	host.accept();
	const closed = new Promise<number>((r) => host.addEventListener('close', (e: CloseEvent) => r(e.code)));
	host.send('x'.repeat(MAX_FRAME_BYTES + 1));
	expect(await closed).toBe(4009);
});

it('rejects a frame by UTF-8 byte length, not UTF-16 code-unit length', async () => {
	const room = '7'.repeat(32);
	const host = (await open(room, 'host')).webSocket!;
	host.accept();
	const oversized = '€'.repeat(2048);
	expect(oversized.length).toBeLessThan(MAX_FRAME_BYTES);
	expect(new TextEncoder().encode(oversized).byteLength).toBeGreaterThan(MAX_FRAME_BYTES);
	const closed = new Promise<number>((r) => host.addEventListener('close', (e: CloseEvent) => r(e.code)));
	host.send(oversized);
	expect(await closed).toBe(4009);
});

it('arms exactly one alarm and expires the room when it fires', async () => {
	const room = '4'.repeat(32);
	await open(room, 'host');
	const roomStub = stubFor(room);
	await open(room, 'guest');
	expect(await runDurableObjectAlarm(roomStub)).toBe(true);
	await runInDurableObject(roomStub, async (_i, state) => {
		expect(await state.storage.getAlarm()).toBeNull();
		expect([...(await state.storage.list())].length).toBe(0);
	});
});

it('keeps admission counting across a reconnect', async () => {
	const room = '5'.repeat(32);
	for (let i = 0; i < ADMISSION.maxJoinsPerRoom; i++) {
		const res = await open(room, 'host');
		expect(res.status).toBe(101);
		res.webSocket!.accept();
		res.webSocket!.close();
		await waitForRoleFree(room, 'host');
	}
	expect((await open(room, 'host')).status).toBe(429);
});

it('survives eviction with hibernated sockets', async () => {
	const room = '6'.repeat(32);
	const host = (await open(room, 'host')).webSocket!;
	host.accept();
	await evictDurableObject(stubFor(room));

	const guest = (await open(room, 'guest')).webSocket!;
	guest.accept();
	const seen = new Promise<string>((r) => guest.addEventListener('message', (e: MessageEvent) => r(e.data as string)));
	host.send('still-here');
	expect(await seen).toBe('still-here');
});

it('a meet room holds no alarm while its host is attached', async () => {
	const room = '8'.repeat(32);
	const host = (await open(room, 'host', 'meet')).webSocket!;
	host.accept();
	await runInDurableObject(stubFor(room), async (_instance, state) => {
		expect(await state.storage.getAlarm()).toBeNull();
	});
});

it('a meet room arms one cleanup alarm when the host leaves and clears it on return', async () => {
	const room = '9'.repeat(32);
	const roomStub = stubFor(room);
	const host = (await open(room, 'host', 'meet')).webSocket!;
	host.accept();
	host.close();
	await waitForRoleFree(room, 'host');
	await runInDurableObject(roomStub, async (_instance, state) => {
		expect(await state.storage.getAlarm()).not.toBeNull();
	});

	const rejoined = (await open(room, 'host', 'meet')).webSocket!;
	rejoined.accept();
	await runInDurableObject(roomStub, async (_instance, state) => {
		expect(await state.storage.getAlarm()).toBeNull();
	});
});

it('a pair room still expires on the pairing window', async () => {
	const room = 'a'.repeat(32);
	await open(room, 'host');
	await runInDurableObject(stubFor(room), async (_instance, state) => {
		expect(await state.storage.getAlarm()).not.toBeNull();
	});
});

it('mismatched kind on an existing room is refused', async () => {
	const room = 'b'.repeat(32);
	await open(room, 'host', 'meet');
	expect((await open(room, 'guest', 'pair')).status).toBe(400);
});

it('a hostless meet room still arms a cleanup alarm for a guest-only join', async () => {
	const room = 'c'.repeat(32);
	await open(room, 'guest', 'meet');
	await runInDurableObject(stubFor(room), async (_instance, state) => {
		expect(await state.storage.getAlarm()).not.toBeNull();
	});
});

it('a meet room arms its cleanup alarm when the host socket errors', async () => {
	const room = 'f'.repeat(32);
	const roomStub = stubFor(room);
	const host = (await open(room, 'host', 'meet')).webSocket!;
	host.accept();

	const errored = await runInDurableObject(roomStub, async (_instance, state) => state.getWebSockets('host')[0]);
	host.close();
	await waitForRoleFree(room, 'host');
	await runInDurableObject(roomStub, async (_instance, state) => {
		await state.storage.deleteAlarm();
		expect(await state.storage.getAlarm()).toBeNull();
	});

	await runInDurableObject(roomStub, async (instance, state) => {
		await instance.webSocketError(errored);
		expect(await state.storage.getAlarm()).not.toBeNull();
	});
});

it('a host joining after a guest-only meet room clears the cleanup alarm', async () => {
	const room = 'd'.repeat(32);
	await open(room, 'guest', 'meet');
	const res = await open(room, 'host', 'meet');
	expect(res.status).toBe(101);
	await runInDurableObject(stubFor(room), async (_instance, state) => {
		expect(await state.storage.getAlarm()).toBeNull();
	});
});
