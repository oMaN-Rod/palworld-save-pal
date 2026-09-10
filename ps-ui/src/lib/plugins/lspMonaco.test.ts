import { describe, expect, it } from 'vitest';
import {
	completionKindFor,
	isLspCompletionItem,
	lspCompletionEntries,
	lspDocumentationText,
	lspHoverText,
	toCompletionSuggestions,
	toMonacoHover
} from './lspMonaco';

describe('isLspCompletionItem', () => {
	it('accepts an object with a string label', () => {
		expect(isLspCompletionItem({ label: 'save' })).toBe(true);
	});

	it('rejects an object without a label', () => {
		expect(isLspCompletionItem({ detail: 'x' })).toBe(false);
	});

	it('rejects a non-object', () => {
		expect(isLspCompletionItem('save')).toBe(false);
		expect(isLspCompletionItem(null)).toBe(false);
		expect(isLspCompletionItem(undefined)).toBe(false);
	});
});

describe('lspCompletionEntries', () => {
	it('accepts a bare array response', () => {
		expect(lspCompletionEntries([{ label: 'a' }, { label: 'b' }]).map((i) => i.label)).toEqual([
			'a',
			'b'
		]);
	});

	it('accepts an {isIncomplete, items} response', () => {
		expect(
			lspCompletionEntries({ isIncomplete: false, items: [{ label: 'a' }] }).map((i) => i.label)
		).toEqual(['a']);
	});

	it('treats null as no suggestions', () => {
		expect(lspCompletionEntries(null)).toEqual([]);
	});

	it('treats an absent result as no suggestions', () => {
		expect(lspCompletionEntries(undefined)).toEqual([]);
	});

	it('drops a malformed entry rather than throwing', () => {
		expect(
			lspCompletionEntries([{ label: 'a' }, { detail: 'no label' }, null, 'nope']).map(
				(i) => i.label
			)
		).toEqual(['a']);
	});
});

describe('completionKindFor', () => {
	it('maps a known kind', () => {
		expect(completionKindFor(3)).toBe('function');
	});

	it('falls back to text for an unmapped kind', () => {
		expect(completionKindFor(11)).toBe('text');
	});

	it('falls back to text for an out-of-range kind', () => {
		expect(completionKindFor(9999)).toBe('text');
	});

	it('falls back to text for an absent kind', () => {
		expect(completionKindFor(undefined)).toBe('text');
	});
});

describe('lspDocumentationText', () => {
	it('passes through a plain string', () => {
		expect(lspDocumentationText('plain')).toBe('plain');
	});

	it('reads a MarkupContent value', () => {
		expect(lspDocumentationText({ value: 'markup' })).toBe('markup');
	});

	it('is empty for an absent value', () => {
		expect(lspDocumentationText(undefined)).toBe('');
	});
});

describe('toCompletionSuggestions', () => {
	it('inserts the label when no insertText or textEdit is given', () => {
		const [entry] = toCompletionSuggestions([{ label: 'save' }]);
		expect(entry.insertText).toBe('save');
		expect(entry.range).toBeNull();
	});

	it('marks a snippet item and is not a snippet otherwise', () => {
		const [snippet, plain] = toCompletionSuggestions([
			{ label: 'for', insertTextFormat: 2 },
			{ label: 'save', insertTextFormat: 1 }
		]);
		expect(snippet.isSnippet).toBe(true);
		expect(plain.isSnippet).toBe(false);
	});

	it('prefers a textEdit range and text over insertText and the word range', () => {
		const range = { start: { line: 1, character: 0 }, end: { line: 1, character: 4 } };
		const [entry] = toCompletionSuggestions([
			{
				label: 'save',
				insertText: 'ignored',
				textEdit: { range, newText: 'save.read' }
			}
		]);
		expect(entry.insertText).toBe('save.read');
		expect(entry.range).toEqual(range);
	});

	it('reads the insert range from an InsertReplaceEdit', () => {
		const insert = { start: { line: 2, character: 0 }, end: { line: 2, character: 2 } };
		const replace = { start: { line: 2, character: 0 }, end: { line: 2, character: 5 } };
		const [entry] = toCompletionSuggestions([
			{ label: 'save', textEdit: { insert, replace, newText: 'save' } }
		]);
		expect(entry.range).toEqual(insert);
	});
});

describe('lspHoverText', () => {
	it('reads a plain string', () => {
		expect(lspHoverText('hello')).toBe('hello');
	});

	it('joins an array of contents', () => {
		expect(lspHoverText(['a', 'b'])).toBe('a\n\nb');
	});

	it('reads a MarkupContent-shaped value', () => {
		expect(lspHoverText({ value: 'markup' })).toBe('markup');
	});

	it('is empty for content with no text', () => {
		expect(lspHoverText(undefined)).toBe('');
		expect(lspHoverText({})).toBe('');
	});
});

describe('toMonacoHover', () => {
	it('wraps hover text into Monaco markdown contents', () => {
		expect(toMonacoHover({ contents: 'a global' })).toEqual({ contents: [{ value: 'a global' }] });
	});

	it('is null when the contents carry no text', () => {
		expect(toMonacoHover({ contents: '' })).toBeNull();
		expect(toMonacoHover(null)).toBeNull();
	});
});
