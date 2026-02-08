import { appStateHandlers } from './appStateHandler';
import { gpsHandlers } from './gpsHandler';
import { guildHandlers } from './guildHandler';
import { lazyLoadHandlers } from './lazyLoadHandler';
import { palHandlers } from './palHandler';
import { playerHandlers } from './playerHandler';
import { presetHandlers } from './presetHandler';
import { saveFileHandlers } from './saveFileHandler';
import { upsHandlers } from './upsHandler';

export const handlers = [
	...appStateHandlers,
	...saveFileHandlers,
	...palHandlers,
	...playerHandlers,
	...guildHandlers,
	...presetHandlers,
	...gpsHandlers,
	...upsHandlers,
	...lazyLoadHandlers
];
