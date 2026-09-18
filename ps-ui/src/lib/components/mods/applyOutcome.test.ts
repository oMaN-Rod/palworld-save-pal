import { describe, expect, it } from 'vitest';
import { applyResult, noOps } from './__tests__/fixtures';
import { appliedStats, isFailed, isNotable } from './applyOutcome';

describe('isFailed', () => {
	it('is true for an error or a mid-apply stop', () => {
		expect(isFailed(applyResult())).toBe(false);
		expect(isFailed(applyResult({ mid_apply: true }))).toBe(true);
		expect(isFailed(applyResult({ error: { code: 'io', message: 'x' } }))).toBe(true);
	});
});

describe('isNotable', () => {
	it('is false for a plain success', () => {
		expect(isNotable(applyResult({ counts: { ...noOps, add: 3 } }))).toBe(false);
	});

	it.each([
		['a failure', { mid_apply: true }],
		['files needing attention', { needs_attention: [{ path: 'a', reason: 'drift' as const }] }],
		['preserved edits', { preserved: ['a'] }],
		['new copies', { new_copies: ['a.new'] }],
		['skipped new copies', { skipped_new_copies: ['a.new'] }],
		['a backup', { backup_dir: 'C:/backups/1' }]
	])('is true for %s', (_, overrides) => {
		expect(isNotable(applyResult(overrides))).toBe(true);
	});
});

describe('appliedStats', () => {
	it('groups the applied operations the way the plan summary does, leaving out empty ones', () => {
		expect(
			appliedStats({ ...noOps, keep: 9, add: 2, replace: 1, reattribute: 1, remove: 3 })
		).toEqual([
			{ label: 'Added', count: 2 },
			{ label: 'Updated', count: 2 },
			{ label: 'Removed', count: 3 }
		]);
	});

	it('counts moves and forgotten edited files', () => {
		expect(appliedStats({ ...noOps, move: 1, remove_preserve: 2 })).toEqual([
			{ label: 'Moved', count: 1 },
			{ label: 'Removed', count: 2 }
		]);
	});
});
