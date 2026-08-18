import { beforeEach, describe, expect, it, vi } from 'vitest';

const validate = vi.fn();
const place = vi.fn();
vi.mock('$lib/data/blueprints.svelte', () => ({
	blueprintsData: {
		validate: (...a: unknown[]) => validate(...a),
		place: (...a: unknown[]) => place(...a)
	}
}));

import { placementState } from './placement.svelte';

beforeEach(() => {
	validate.mockReset();
	place.mockReset();
	placementState.exit();
});

describe('placementState', () => {
	it('enter sets the session active with the handle and a default anchor', () => {
		placementState.enter('h1', { name: 'Home' } as any);
		expect(placementState.active).toBe(true);
		expect(placementState.handle).toBe('h1');
		expect(placementState.anchor).toMatchObject({ yaw: 0 });
	});

	it('runValidate sends the current anchor + guild and stores findings/hasBlocking', async () => {
		placementState.enter('h1', { name: 'Home' } as any);
		placementState.setAnchor({ x: 5, y: 6, z: 7, yaw: 0.5 });
		placementState.targetGuild = 'guild-9';
		validate.mockResolvedValueOnce({
			findings: [{ severity: 'blocking', code: 'base_limit', message: 'too many' }],
			has_blocking: true
		});
		await placementState.runValidate();
		expect(validate).toHaveBeenCalledWith('h1', { x: 5, y: 6, z: 7, yaw: 0.5 }, 'guild-9');
		expect(placementState.hasBlocking).toBe(true);
		expect(placementState.findings).toHaveLength(1);
	});

	it('runValidate is a no-op without a target guild (nothing to validate against)', async () => {
		placementState.enter('h1', { name: 'Home' } as any);
		placementState.targetGuild = '';
		await placementState.runValidate();
		expect(validate).not.toHaveBeenCalled();
	});

	it('commit places with the anchor, targets and override flag', async () => {
		placementState.enter('h1', { name: 'Home' } as any);
		placementState.setAnchor({ x: 1, y: 2, z: 3, yaw: 0 });
		placementState.targetGuild = 'guild-9';
		placementState.targetPlayer = 'player-2';
		placementState.overrideWarnings = true;
		place.mockResolvedValueOnce({ base_id: 'base-1', structures_placed: 8, findings: [] });
		const res = await placementState.commit();
		expect(place).toHaveBeenCalledWith(
			'h1',
			{ x: 1, y: 2, z: 3, yaw: 0 },
			'guild-9',
			'player-2',
			true
		);
		expect(res.base_id).toBe('base-1');
	});

	it('exit clears the session', () => {
		placementState.enter('h1', { name: 'Home' } as any);
		placementState.exit();
		expect(placementState.active).toBe(false);
		expect(placementState.handle).toBeNull();
	});
});
