import { appStateHandlers } from './appStateHandler';
import { blueprintHandlers } from './blueprintHandler';
import { gpsHandlers } from './gpsHandler';
import { guildHandlers } from './guildHandler';
import { lazyLoadHandlers } from './lazyLoadHandler';
import { palHandlers } from './palHandler';
import { playerHandlers } from './playerHandler';
import { presetHandlers } from './presetHandler';
import { saveFileHandlers } from './saveFileHandler';
import { upsHandlers } from './upsHandler';
import { serverHandlers } from './serverHandler';

export const handlers = [
	...appStateHandlers,
	...blueprintHandlers,
	...saveFileHandlers,
	...palHandlers,
	...playerHandlers,
	...guildHandlers,
	...presetHandlers,
	...gpsHandlers,
	...upsHandlers,
	...lazyLoadHandlers,
	...serverHandlers
];
