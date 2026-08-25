import type { LspRange } from './lspClient';

export interface LspCompletionItem {
	label: string;
	kind?: number;
	detail?: string;
	documentation?: string | { value?: string };
	insertText?: string;
	insertTextFormat?: number;
	textEdit?:
		| { range: LspRange; newText: string }
		| { insert: LspRange; replace: LspRange; newText: string };
}

export function isLspCompletionItem(value: unknown): value is LspCompletionItem {
	return (
		typeof value === 'object' &&
		value !== null &&
		typeof (value as LspCompletionItem).label === 'string'
	);
}

/** LuaLS may answer with a bare array or a `{isIncomplete, items}` list; both are valid completion results. */
export function lspCompletionEntries(raw: unknown): LspCompletionItem[] {
	if (Array.isArray(raw)) return raw.filter(isLspCompletionItem);
	const items = (raw as { items?: unknown } | null)?.items;
	return Array.isArray(items) ? items.filter(isLspCompletionItem) : [];
}

export function lspDocumentationText(doc: LspCompletionItem['documentation']): string {
	if (typeof doc === 'string') return doc;
	return typeof doc?.value === 'string' ? doc.value : '';
}

export type LspCompletionKind =
	| 'text'
	| 'method'
	| 'function'
	| 'constructor'
	| 'field'
	| 'variable'
	| 'class'
	| 'interface'
	| 'module'
	| 'property'
	| 'enum'
	| 'keyword'
	| 'snippet'
	| 'enumMember'
	| 'constant'
	| 'struct'
	| 'event'
	| 'operator'
	| 'typeParameter';

/** LSP's `CompletionItemKind` is 1-based; a kind this map does not name, or an absent one, resolves to `'text'`. */
const LSP_COMPLETION_KINDS: Record<number, LspCompletionKind> = {
	1: 'text',
	2: 'method',
	3: 'function',
	4: 'constructor',
	5: 'field',
	6: 'variable',
	7: 'class',
	8: 'interface',
	9: 'module',
	10: 'property',
	13: 'enum',
	14: 'keyword',
	15: 'snippet',
	20: 'enumMember',
	21: 'constant',
	22: 'struct',
	23: 'event',
	24: 'operator',
	25: 'typeParameter'
};

export function completionKindFor(kind: number | undefined): LspCompletionKind {
	if (kind === undefined) return 'text';
	return LSP_COMPLETION_KINDS[kind] ?? 'text';
}

export interface LspCompletionSuggestion {
	label: string;
	kind: LspCompletionKind;
	detail: string;
	documentation: string;
	insertText: string;
	isSnippet: boolean;
	range: LspRange | null;
}

/**
 * Prefers an item's own `textEdit` range and text over the caller's computed
 * word range: LuaLS sends one whenever the edit doesn't match the simple
 * "replace the word under the cursor" case, e.g. a snippet with placeholders.
 */
export function toCompletionSuggestions(raw: unknown): LspCompletionSuggestion[] {
	return lspCompletionEntries(raw).map((item) => {
		const edit = item.textEdit;
		const editRange = edit ? ('range' in edit ? edit.range : edit.insert) : null;
		return {
			label: item.label,
			kind: completionKindFor(item.kind),
			detail: item.detail ?? '',
			documentation: lspDocumentationText(item.documentation),
			insertText: edit ? edit.newText : (item.insertText ?? item.label),
			isSnippet: item.insertTextFormat === 2,
			range: editRange
		};
	});
}

export function lspHoverText(contents: unknown): string {
	if (typeof contents === 'string') return contents;
	if (Array.isArray(contents)) return contents.map(lspHoverText).join('\n\n');
	const value = (contents as { value?: unknown } | null)?.value;
	return typeof value === 'string' ? value : '';
}

export function toMonacoHover(raw: unknown): import('monaco-editor').languages.Hover | null {
	const contents = (raw as { contents?: unknown } | null)?.contents;
	const text = lspHoverText(contents);
	return text ? { contents: [{ value: text }] } : null;
}
