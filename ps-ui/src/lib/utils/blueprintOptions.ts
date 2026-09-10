import type { CaptureOptions } from '$types';

export type CapturePreset = 'blueprint' | 'configured' | 'full';

const NONE: CaptureOptions = {
	production_config: false,
	structure_condition: false,
	container_contents: false,
	worker_pals: false,
	housed_pals: false,
	production_progress: false,
	access_config: false,
	base_identity: false
};

export function captureOptionsForPreset(preset: CapturePreset): CaptureOptions {
	switch (preset) {
		case 'blueprint':
			return { ...NONE, production_config: true };
		case 'configured':
			return {
				...NONE,
				production_config: true,
				structure_condition: true,
				access_config: true,
				base_identity: true
			};
		case 'full':
			return {
				production_config: true,
				structure_condition: true,
				container_contents: true,
				worker_pals: true,
				housed_pals: true,
				production_progress: true,
				access_config: true,
				base_identity: true
			};
	}
}

export const CAPTURE_OPTION_FIELDS: {
	key: keyof CaptureOptions;
	label: string;
	description: string;
}[] = [
	{ key: 'production_config', label: 'Production config', description: 'Recipes and work assignments set on production structures.' },
	{ key: 'structure_condition', label: 'Structure condition', description: 'Current HP / damage state of each structure.' },
	{ key: 'container_contents', label: 'Container contents', description: 'Items stored in chests and containers.' },
	{ key: 'worker_pals', label: 'Worker pals', description: 'Pals assigned to work at the base.' },
	{ key: 'housed_pals', label: 'Housed pals', description: 'Pals living in the base (palboxes, beds).' },
	{ key: 'production_progress', label: 'Production progress', description: 'In-progress crafting and smelting timers.' },
	{ key: 'access_config', label: 'Access config', description: 'Locks and permission settings (passwords are never captured except in Full).' },
	{ key: 'base_identity', label: 'Base identity', description: 'Original base and world names (owner UIDs are always anonymized).' }
];
