import type { goto } from '$app/navigation';
import type { Message } from '../../../../shared/types';

export interface WSHandlerContext {
	goto: typeof goto;
	reset?: () => void;
}

export interface WSMessageHandler {
	type: string;
	handle: (data: any, context: WSHandlerContext) => Promise<void>;
}

export interface WSDispatcher {
	register: (handler: WSMessageHandler) => void;
	dispatch: (message: Message, context: WSHandlerContext) => Promise<void>;
}
