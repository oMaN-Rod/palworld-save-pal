// Async wrapper around decodeSceneryStream: runs the parse in a one-shot Web
// Worker so the main thread never blocks on the ~50k-instance decode, falling
// back to the inline decode wherever Workers are unavailable (SSR, older
// webviews, and the unit-test environment -- `Worker` is undefined there).
import { decodeSceneryStream, type SceneryStream } from './sceneryFormat';

type StreamMessage = { stream: SceneryStream | null; error: string | null };

export async function decodeSceneryStreamAsync(buffer: ArrayBuffer): Promise<SceneryStream> {
	if (typeof Worker === 'undefined') return decodeSceneryStream(buffer);

	const worker = new Worker(new URL('./sceneryStream.worker.ts', import.meta.url), {
		type: 'module'
	});
	try {
		return await new Promise<SceneryStream>((resolve, reject) => {
			worker.onmessage = (event: MessageEvent<StreamMessage>) => {
				const message = event.data;
				if (message.error !== null && message.error !== undefined) {
					reject(new Error(message.error));
				} else if (message.stream) {
					resolve(message.stream);
				} else {
					reject(new Error('scenery worker returned neither a stream nor an error'));
				}
			};
			worker.onerror = (event) => reject(new Error(event.message || 'scenery worker failed'));
			worker.postMessage(buffer, [buffer]);
		});
	} finally {
		worker.terminate();
	}
}
