import type { Message } from '$types';
import type { WSDispatcher, WSHandlerContext, WSMessageHandler } from './types';

export class MessageDispatcher implements WSDispatcher {
	private handlers = new Map<string, WSMessageHandler>();

	register(handler: WSMessageHandler): void {
		this.handlers.set(handler.type, handler);
	}

	async dispatch(message: Message, context: WSHandlerContext): Promise<void> {
		const handler = this.handlers.get(message.type);
		if (handler) {
			await handler.handle(message.data, context);
		} else {
			console.warn(`No handler registered for message type: ${message.type}`);
		}
	}
}

let dispatcherInstance: MessageDispatcher | null = null;

export function getDispatcher(): MessageDispatcher {
	if (!dispatcherInstance) {
		dispatcherInstance = new MessageDispatcher();
	}
	return dispatcherInstance;
}
