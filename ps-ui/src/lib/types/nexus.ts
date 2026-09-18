import type { Decision, InstallManifest, ModError } from './mods';

export interface NexusAccount {
	user_id: number;
	name: string;
	is_premium: boolean;
	is_supporter: boolean;
	profile_url: string | null;
}

export interface NexusRateLimit {
	hourly_limit: number | null;
	hourly_remaining: number | null;
	hourly_reset: string | null;
	daily_limit: number | null;
	daily_remaining: number | null;
	daily_reset: string | null;
}

export interface NexusAccountReply {
	has_key: boolean;
	account: NexusAccount | null;
	rate_limit: NexusRateLimit | null;
	error?: ModError;
}

export interface NexusCategory {
	category_id: number;
	name: string;
	parent_category: number | null;
}

export interface NexusCategoriesReply {
	categories: NexusCategory[];
	error?: ModError;
}

export type NexusSort = 'relevance' | 'downloads' | 'endorsements' | 'updated' | 'created' | 'name';

export interface NexusSearchParams {
	query?: string;
	category?: string;
	sort?: NexusSort;
	offset?: number;
	count?: number;
	include_adult?: boolean;
}

export interface NexusModSummary {
	mod_id: number;
	name: string;
	summary: string | null;
	version: string | null;
	author: string | null;
	uploader: { name: string } | null;
	picture_url: string | null;
	thumbnail_url: string | null;
	endorsements: number;
	downloads: number;
	file_size: number | null;
	adult_content: boolean;
	created_at: string | null;
	updated_at: string | null;
	category: string | null;
}

export interface NexusSearchReply {
	offset: number;
	count: number;
	total_count: number;
	mods: NexusModSummary[];
	error?: ModError;
}

export type NexusFileCategory =
	| 'MAIN'
	| 'UPDATE'
	| 'OPTIONAL'
	| 'OLD_VERSION'
	| 'MISCELLANEOUS'
	| 'REMOVED'
	| 'ARCHIVED'
	| 'UNKNOWN';

export interface NexusFile {
	file_id: number;
	name: string;
	version: string;
	category: NexusFileCategory;
	date: number;
	size_in_bytes: number | null;
	uri: string;
	primary: boolean;
	description: string | null;
}

export interface NexusFilesReply {
	mod_id: number;
	files: NexusFile[];
	latest_file_id: number | null;
	error?: ModError;
}

export interface NexusDownloadRequest {
	target_id: string;
	mod_id: number;
	file_id: number;
	key?: string;
	expires?: number;
	accept_defaults?: boolean;
	enable?: boolean;
}

/** Installed, needs_decisions and refused all arrive under this one type. */
export interface NexusDownloadReply {
	target_id: string;
	nexus_mod_id: number;
	file_id: number;
	version?: string;
	file_name?: string;
	mod_id?: string;
	version_id?: string;
	manifest?: InstallManifest;
	enable_error?: ModError | null;
	needs_decisions?: Decision[];
	error?: ModError;
}

export interface NxmLink {
	mod_id: number;
	file_id: number;
	key: string | null;
	expires: number | null;
}

/** `{link}`, `{link, error}` when expired, or `{error}` when rejected. */
export interface NexusLinkPush {
	link?: NxmLink;
	error?: ModError;
}

export interface NexusHandlerStatus {
	supported: boolean;
	registered: boolean;
	foreign: boolean;
	current: string | null;
	error?: ModError;
}

export type UpdateState = 'up_to_date' | 'available' | 'ignored';

export interface ModUpdate {
	mod_id: string;
	nexus_mod_id: number;
	installed_version: string | null;
	latest: NexusFile | null;
	state: UpdateState;
	ignored_version: string | null;
}

export interface ModUpdateCheckReply {
	target_id: string | null;
	checked: number;
	truncated: boolean;
	updates: ModUpdate[];
	error?: ModError;
}

export interface ModUpdateIgnoreReply {
	mod_id: string;
	ignored_version: string | null;
	error?: ModError;
}
