import { appStateHandlers } from './appStateHandler';
import { guildHandlers } from './guildHandler';
import { palHandlers } from './palHandler';
import { playerHandlers } from './playerHandler';
import { saveFileHandlers } from './saveFileHandler';

export const handlers = [
	...appStateHandlers,
	...saveFileHandlers,
	...palHandlers,
	...playerHandlers,
	...guildHandlers
];
