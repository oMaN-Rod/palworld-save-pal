import { getLiveActors } from '$lib/data/liveActors.svelte';
import type { LiveFrameJson } from '$lib/signal/session.svelte';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

export const liveFrameHandler: WSMessageHandler = {
	type: MessageType.LIVE_FRAME,
	async handle(data: LiveFrameJson) {
		getLiveActors().applyFrame(data);
	}
};

export const liveHandlers = [liveFrameHandler];
