// Decodes a scenery instance stream off the main thread. The decode of
// ~50k instances (DataView reads + typed-array fills) takes tens to hundreds
// of milliseconds synchronously — on Firefox that surfaces as a hard freeze
// right when 3D turns on. The buffer is transferred in, so this costs no copy.
import { decodeSceneryStream } from './sceneryFormat';

self.onmessage = (event: MessageEvent<ArrayBuffer>) => {
	try {
		const stream = decodeSceneryStream(event.data);
		(self as unknown as Worker).postMessage({ stream, error: null });
	} catch (error) {
		(self as unknown as Worker).postMessage({
			stream: null,
			error: error instanceof Error ? error.message : String(error)
		});
	}
};
