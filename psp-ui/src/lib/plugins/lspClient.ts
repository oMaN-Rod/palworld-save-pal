import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

export type EditorTier = 'full' | 'starting' | 'baseline';

export interface TierStatus {
	tier: EditorTier;
	reason: string | null;
}

export interface LspPosition {
	line: number;
	character: number;
}

export interface LspRange {
	start: LspPosition;
	end: LspPosition;
}

export interface MonacoRange {
	startLineNumber: number;
	startColumn: number;
	endLineNumber: number;
	endColumn: number;
}

export type DiagnosticSeverity = 'error' | 'warning' | 'info' | 'hint';

export interface LspDiagnostic {
	range: MonacoRange;
	severity: DiagnosticSeverity;
	message: string;
}

type DiagnosticsCallback = (uri: string, diagnostics: LspDiagnostic[]) => void;

/** Monaco positions are 1-based; LSP positions are 0-based. */
export function positionToLsp(line: number, column: number): LspPosition {
	return { line: line - 1, character: column - 1 };
}

export function rangeFromLsp(range: LspRange): MonacoRange {
	return {
		startLineNumber: range.start.line + 1,
		startColumn: range.start.character + 1,
		endLineNumber: range.end.line + 1,
		endColumn: range.end.character + 1
	};
}

const SEVERITIES: Record<number, DiagnosticSeverity> = {
	1: 'error',
	2: 'warning',
	3: 'info',
	4: 'hint'
};

/** LSP omits `severity` to mean error, which is also the fallback for a value it never defined. */
export function severityFromLsp(severity: number | undefined): DiagnosticSeverity {
	return SEVERITIES[severity ?? 1] ?? 'error';
}

interface JsonRpcError {
	code?: number;
	message: string;
}

interface LspResponseFrame {
	jsonrpc?: string;
	id?: number;
	result?: unknown;
	error?: JsonRpcError;
}

export interface LspRequestReply {
	request_id?: string;
	frame?: LspResponseFrame;
	error?: string;
}

interface OpenSessionResponse {
	root_uri?: string | null;
	error?: string;
}

interface LspDiagnosticParam {
	range: LspRange;
	severity?: number;
	message: string;
}

/**
 * Exactly the characters the server percent-encodes when it builds the
 * workspace uri. Encoding more (or fewer) would name a document the language
 * server does not recognise as the file it indexed.
 */
const RESERVED_URI_CHARACTERS = ' "#%<>?`{}';

function encodeUriPath(path: string): string {
	return Array.from(path, (character) => {
		const code = character.codePointAt(0) ?? 0;
		return code < 0x20 || code === 0x7f || RESERVED_URI_CHARACTERS.includes(character)
			? encodeURIComponent(character)
			: character;
	}).join('');
}

function decodeUriPath(path: string): string {
	try {
		return decodeURIComponent(path);
	} catch {
		return path;
	}
}

function positionParams(uri: string, line: number, column: number) {
	return { textDocument: { uri }, position: positionToLsp(line, column) };
}

interface PendingRequest {
	resolve: (value: unknown) => void;
	reject: (reason: Error) => void;
}

/**
 * Thin transport shim over the plugin LSP wire messages. It owns no retry or
 * timeout logic: the server always answers under the request's own message
 * type, so nothing here is left waiting on a frame that never comes.
 */
export class LspClient {
	#pluginId: string | null = null;
	#rootUri: string | null = null;
	#onDiagnostics: DiagnosticsCallback | null = null;
	#versions = new Map<string, number>();
	/**
	 * Keyed by a request id this client generated, because the transport's own
	 * queue is keyed by message type: two LSP requests in flight at once — a
	 * hover raised while a references search runs, which Monaco does as a
	 * matter of course — would otherwise overwrite each other's resolver.
	 */
	#pending = new Map<string, PendingRequest>();

	get pluginId(): string | null {
		return this.#pluginId;
	}

	get rootUri(): string | null {
		return this.#rootUri;
	}

	async probe(): Promise<TierStatus> {
		return sendAndWait<TierStatus>(MessageType.GET_EDITOR_TIER);
	}

	/**
	 * Starts the plugin's language server and adopts the workspace it indexed.
	 * Every document uri sent afterwards is built from that root, so the server
	 * is asked about the files it actually has on disk.
	 */
	async open(pluginId: string, sources: Record<string, string>): Promise<void> {
		this.#failPending('the language server session was replaced');
		const response = await sendAndWait<OpenSessionResponse>(MessageType.OPEN_LSP_SESSION, {
			plugin_id: pluginId
		});
		if (response.error) throw new Error(response.error);
		if (!response.root_uri) {
			throw new Error(`the language server opened no workspace for ${pluginId}`);
		}

		this.#pluginId = pluginId;
		this.#rootUri = response.root_uri;
		this.#versions.clear();
		for (const [path, text] of Object.entries(sources)) {
			this.#versions.set(path, 1);
			this.#notify('textDocument/didOpen', {
				textDocument: { uri: this.uriFor(path), languageId: 'lua', version: 1, text }
			});
		}
	}

	uriFor(path: string): string {
		return `${this.#requireRootUri()}/${encodeUriPath(path)}`;
	}

	/** The inverse of `uriFor`: `null` for a uri outside this plugin's workspace. */
	pathFor(uri: string): string | null {
		if (!this.#rootUri) return null;
		const prefix = `${this.#rootUri}/`;
		if (!uri.startsWith(prefix)) return null;
		return decodeUriPath(uri.slice(prefix.length));
	}

	/** LSP requires a strictly increasing version per document; a server that honours it may drop stale edits otherwise. */
	didChange(path: string, text: string): void {
		if (!this.#pluginId) return;
		const version = (this.#versions.get(path) ?? 1) + 1;
		this.#versions.set(path, version);
		this.#notify('textDocument/didChange', {
			textDocument: { uri: this.uriFor(path), version },
			contentChanges: [{ text }]
		});
	}

	definition(path: string, line: number, column: number): Promise<unknown> {
		return this.#requestAtPosition('textDocument/definition', path, line, column);
	}

	hover(path: string, line: number, column: number): Promise<unknown> {
		return this.#requestAtPosition('textDocument/hover', path, line, column);
	}

	completion(path: string, line: number, column: number): Promise<unknown> {
		return this.#requestAtPosition('textDocument/completion', path, line, column);
	}

	rename(path: string, line: number, column: number, newName: string): Promise<unknown> {
		return this.#requestAtPosition('textDocument/rename', path, line, column, { newName });
	}

	references(path: string, line: number, column: number): Promise<unknown> {
		return this.#requestAtPosition('textDocument/references', path, line, column, {
			context: { includeDeclaration: true }
		});
	}

	/** `uriFor` throws synchronously when no session is open; routing every position request through here turns that into an ordinary rejection instead of an exception the caller's `.catch` never sees. */
	#requestAtPosition(
		method: string,
		path: string,
		line: number,
		column: number,
		extra?: Record<string, unknown>
	): Promise<unknown> {
		if (!this.#rootUri) {
			return Promise.reject(new Error('LspClient.open() must be called before making requests'));
		}
		return this.#request(method, { ...positionParams(this.uriFor(path), line, column), ...extra });
	}

	onDiagnostics(cb: DiagnosticsCallback): void {
		this.#onDiagnostics = cb;
	}

	dispose(): void {
		this.#pluginId = null;
		this.#rootUri = null;
		this.#onDiagnostics = null;
		this.#versions.clear();
		this.#failPending('the language server session was closed');
	}

	/**
	 * Nothing times a request out, so a request whose reply never comes is only
	 * ever settled here. Leaving one pending across a session change would
	 * strand its caller for the life of the page.
	 */
	#failPending(reason: string): void {
		const pending = [...this.#pending.values()];
		this.#pending.clear();
		for (const request of pending) {
			request.reject(new Error(reason));
		}
	}

	/** Called by the `lsp_request` ws handler with the answer to one request this client sent. */
	handleRequestReply(reply: LspRequestReply): void {
		if (typeof reply?.request_id !== 'string') return;
		const request = this.#pending.get(reply.request_id);
		if (!request) return;
		this.#pending.delete(reply.request_id);

		if (reply.error) {
			request.reject(new Error(reply.error));
			return;
		}
		const frameError = reply.frame?.error;
		if (frameError) {
			request.reject(
				new Error(
					frameError.code !== undefined
						? `${frameError.message} (${frameError.code})`
						: frameError.message
				)
			);
			return;
		}
		request.resolve(reply.frame?.result);
	}

	/** Called by the `lsp_notification` ws handler with the raw frame the language server sent unprompted. */
	handleFrame(frame: unknown): void {
		if (!this.#onDiagnostics) return;
		const notification = frame as { method?: unknown; params?: unknown } | null;
		if (
			typeof notification !== 'object' ||
			notification === null ||
			notification.method !== 'textDocument/publishDiagnostics'
		) {
			return;
		}
		const params = notification.params as { uri?: unknown; diagnostics?: unknown } | null;
		if (
			typeof params !== 'object' ||
			params === null ||
			typeof params.uri !== 'string' ||
			!Array.isArray(params.diagnostics)
		) {
			return;
		}
		this.#onDiagnostics(
			params.uri,
			(params.diagnostics as LspDiagnosticParam[]).map((diagnostic) => ({
				range: rangeFromLsp(diagnostic.range),
				severity: severityFromLsp(diagnostic.severity),
				message: diagnostic.message
			}))
		);
	}

	#requireRootUri(): string {
		if (!this.#rootUri) throw new Error('LspClient.open() must be called before making requests');
		return this.#rootUri;
	}

	#request(method: string, params: unknown): Promise<unknown> {
		const requestId = crypto.randomUUID();
		const settled = new Promise<unknown>((resolve, reject) => {
			this.#pending.set(requestId, { resolve, reject });
		});
		send(MessageType.LSP_REQUEST, {
			plugin_id: this.#pluginId,
			request_id: requestId,
			frame: { jsonrpc: '2.0', method, params }
		});
		return settled;
	}

	/** A notification gets no reply on success, so this must not use `sendAndWait` — nothing would ever resolve it. */
	#notify(method: string, params: unknown): void {
		send(MessageType.LSP_NOTIFICATION, {
			plugin_id: this.#pluginId,
			frame: { jsonrpc: '2.0', method, params }
		});
	}
}

export const lspClient = new LspClient();
