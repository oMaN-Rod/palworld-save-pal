import {
	fieldDoc,
	functionDoc,
	globalByName,
	signatureLabel,
	typeName,
	visibleGlobals,
	type ApiDefinition,
	type ApiFunction,
	type ApiGlobal,
	type PluginCapability
} from './apiDefinition';

export interface ApiSnapshot {
	definition: ApiDefinition;
	granted: PluginCapability[];
}

export type CompletionKind = 'module' | 'function' | 'field';

export interface CompletionEntry {
	label: string;
	kind: CompletionKind;
	detail: string;
	documentation: string;
	insertText: string;
}

export interface SignatureInfo {
	label: string;
	documentation: string;
	parameters: { label: string; documentation: string }[];
}

const OWNER = /([A-Za-z_]\w*)\.\w*$/;

export function ownerBeforeCursor(lineUpToCursor: string): string | null {
	return OWNER.exec(lineUpToCursor)?.[1] ?? null;
}

function visibleGlobal(snapshot: ApiSnapshot, name: string): ApiGlobal | undefined {
	return visibleGlobals(snapshot.definition, snapshot.granted).find(
		(global) => global.name === name
	);
}

export function completionItems(
	snapshot: ApiSnapshot,
	owner: string | null,
	fullTierLive = false
): CompletionEntry[] {
	if (fullTierLive) return [];
	if (owner === null) {
		return visibleGlobals(snapshot.definition, snapshot.granted).map((global) => ({
			label: global.name,
			kind: 'module' as const,
			detail: global.capability ? `requires ${global.capability}` : 'always available',
			documentation: '',
			insertText: global.name
		}));
	}

	const global = visibleGlobal(snapshot, owner);
	if (!global) return [];

	const source = globalByName(snapshot.definition, owner)!;
	const functions = global.functions.map((fn) => ({
		label: fn.name,
		kind: 'function' as const,
		detail: signatureLabel(fn),
		documentation: functionDoc(fn, source.capability),
		insertText: fn.name
	}));
	const fields = global.fields.map((field) => ({
		label: field.name,
		kind: 'field' as const,
		detail: typeName(field.type),
		documentation: fieldDoc(field),
		insertText: field.name
	}));
	return [...functions, ...fields];
}

export function hoverFor(
	snapshot: ApiSnapshot,
	owner: string | null,
	word: string,
	fullTierLive = false
): string | null {
	if (fullTierLive) return null;
	if (owner === null) {
		const global = visibleGlobal(snapshot, word);
		if (!global) return null;
		return global.capability
			? `\`${global.name}\` — requires capability \`${global.capability}\`.`
			: `\`${global.name}\``;
	}
	const item = completionItems(snapshot, owner).find((entry) => entry.label === word);
	if (!item) return null;
	return `\`\`\`lua\n${owner}.${item.detail}\n\`\`\`\n\n${item.documentation}`;
}

function visibleFunction(
	snapshot: ApiSnapshot,
	owner: string,
	name: string
): { fn: ApiFunction; ownerCapability: PluginCapability | null } | null {
	const global = visibleGlobal(snapshot, owner);
	const fn = global?.functions.find((candidate) => candidate.name === name);
	if (!fn) return null;
	return { fn, ownerCapability: globalByName(snapshot.definition, owner)!.capability };
}

export function signatureFor(
	snapshot: ApiSnapshot,
	owner: string,
	name: string,
	fullTierLive = false
): SignatureInfo | null {
	void fullTierLive;
	const found = visibleFunction(snapshot, owner, name);
	if (!found) return null;
	return {
		label: signatureLabel(found.fn),
		documentation: functionDoc(found.fn, found.ownerCapability),
		parameters: found.fn.params.map((param) => ({
			label: `${param.name}${param.optional ? '?' : ''}: ${typeName(param.type)}`,
			documentation: param.optional ? 'Optional.' : ''
		}))
	};
}

export function registerLuaProviders(
	monaco: typeof import('monaco-editor'),
	getSnapshot: () => ApiSnapshot,
	isFullTierLive: () => boolean
): { dispose(): void } {
	const kinds: Record<CompletionKind, import('monaco-editor').languages.CompletionItemKind> = {
		module: monaco.languages.CompletionItemKind.Module,
		function: monaco.languages.CompletionItemKind.Function,
		field: monaco.languages.CompletionItemKind.Field
	};

	const completion = monaco.languages.registerCompletionItemProvider('lua', {
		triggerCharacters: ['.'],
		provideCompletionItems(model, position) {
			const line = model.getValueInRange({
				startLineNumber: position.lineNumber,
				startColumn: 1,
				endLineNumber: position.lineNumber,
				endColumn: position.column
			});
			const word = model.getWordUntilPosition(position);
			const range = {
				startLineNumber: position.lineNumber,
				endLineNumber: position.lineNumber,
				startColumn: word.startColumn,
				endColumn: word.endColumn
			};
			return {
				suggestions: completionItems(
					getSnapshot(),
					ownerBeforeCursor(line),
					isFullTierLive()
				).map((entry) => ({
					label: entry.label,
					kind: kinds[entry.kind],
					detail: entry.detail,
					documentation: { value: entry.documentation },
					insertText: entry.insertText,
					range
				}))
			};
		}
	});

	const hover = monaco.languages.registerHoverProvider('lua', {
		provideHover(model, position) {
			const word = model.getWordAtPosition(position);
			if (!word) return null;
			const line = model.getValueInRange({
				startLineNumber: position.lineNumber,
				startColumn: 1,
				endLineNumber: position.lineNumber,
				endColumn: word.startColumn
			});
			const owner = ownerBeforeCursor(`${line}x`);
			const text = hoverFor(getSnapshot(), owner, word.word, isFullTierLive());
			return text ? { contents: [{ value: text }] } : null;
		}
	});

	const signature = monaco.languages.registerSignatureHelpProvider('lua', {
		signatureHelpTriggerCharacters: ['(', ','],
		provideSignatureHelp(model, position) {
			const line = model.getValueInRange({
				startLineNumber: position.lineNumber,
				startColumn: 1,
				endLineNumber: position.lineNumber,
				endColumn: position.column
			});
			const call = /([A-Za-z_]\w*)\.([A-Za-z_]\w*)\s*\([^()]*$/.exec(line);
			if (!call) return null;
			const info = signatureFor(getSnapshot(), call[1], call[2], isFullTierLive());
			if (!info) return null;
			return {
				value: {
					signatures: [
						{
							label: info.label,
							documentation: { value: info.documentation },
							parameters: info.parameters.map((param) => ({
								label: param.label,
								documentation: { value: param.documentation }
							}))
						}
					],
					activeSignature: 0,
					activeParameter: (line.slice(line.lastIndexOf('(') + 1).match(/,/g) ?? []).length
				},
				dispose() {}
			};
		}
	});

	return {
		dispose() {
			completion.dispose();
			hover.dispose();
			signature.dispose();
		}
	};
}
