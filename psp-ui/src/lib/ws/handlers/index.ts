import { appStateHandlers } from './appStateHandler';
import { blueprintHandlers } from './blueprintHandler';
import { gpsHandlers } from './gpsHandler';
import { guildHandlers } from './guildHandler';
import { lazyLoadHandlers } from './lazyLoadHandler';
import { overviewHandlers } from './overviewHandler';
import { palHandlers } from './palHandler';
import { playerHandlers } from './playerHandler';
import { presetHandlers } from './presetHandler';
import { pluginHandlers } from './pluginHandler';
import { saveFileHandlers } from './saveFileHandler';
import { upsHandlers } from './upsHandler';
import { serverHandlers } from './serverHandler';
import { signalHandlers } from './signalHandler';

export const handlers = [
	...appStateHandlers,
	...blueprintHandlers,
	...saveFileHandlers,
	...palHandlers,
	...playerHandlers,
	...guildHandlers,
	...presetHandlers,
	...pluginHandlers,
	...gpsHandlers,
	...upsHandlers,
	...lazyLoadHandlers,
	...overviewHandlers,
	...serverHandlers,
	...signalHandlers
];
