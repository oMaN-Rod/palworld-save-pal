import { appStateHandlers } from './appStateHandler';
import { blueprintHandlers } from './blueprintHandler';
import { gpsHandlers } from './gpsHandler';
import { guildHandlers } from './guildHandler';
import { lazyLoadHandlers } from './lazyLoadHandler';
import { overviewHandlers } from './overviewHandler';
import { palHandlers } from './palHandler';
import { playerHandlers } from './playerHandler';
import { presetHandlers } from './presetHandler';
import { saveFileHandlers } from './saveFileHandler';
import { serverHandlers } from './serverHandler';
import { upsHandlers } from './upsHandler';

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
	...overviewHandlers,
	...serverHandlers
];
