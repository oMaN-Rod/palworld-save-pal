import type { goto } from '$app/navigation';
import type { Message, MessageType } from '$types';

export interface WSHandlerContext {
	goto: typeof goto;
	reset?: () => void;
}

export interface WSMessageHandler {
	type: MessageType;
	handle: (data: any, context: WSHandlerContext) => Promise<void>;
}

export interface WSDispatcher {
	register: (handler: WSMessageHandler) => void;
	dispatch: (message: Message, context: WSHandlerContext) => Promise<void>;
}

export interface Transport {
	readonly kind: 'ws' | 'worker' | 'remote';
	connect(context: WSHandlerContext): void;
	isConnected(): boolean;
	send(messageData: string): Promise<void>;
	sendBytes(type: string, bytes: Uint8Array): Promise<void>;
	sendAndWait(messageData: any): Promise<any>;
	clear(messageType: string): void;
	get message(): Message | null;
	set message(value: Message | null);
	get connected(): boolean;
}
