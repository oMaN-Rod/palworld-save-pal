/**
 * Breeding calculator wire types — mirror the Rust `psp_core::breeding` structs
 * serialized by the psp-app handlers. Kept in sync with the serde shapes.
 */

export interface BreedablePal {
	tribe: string;
	display_name: string;
	icon: string | null;
	combi_rank: number | null;
	rarity: number | null;
	gender_prob: { male: number; female: number };
}

export interface BreedablePalsResponse {
	pals: BreedablePal[];
	total: number;
}

export interface DirectResultItem {
	parent_a: string;
	parent_b: string;
	child: string;
	child_display: string | null;
	child_icon: string | null;
	child_gender_prob: { male: number; female: number } | null;
	combo_type: 'formula' | 'unique';
	/** Set only for the unique combos the game gates on parent gender
	 *  (`DT_PalCombiUnique.ParentGenderA/B`) — e.g. CatMage + FoxMage yields a
	 *  different child depending on which parent is male. Relative to this
	 *  row's own `parent_a`/`parent_b` order. */
	parent_a_gender?: 'Male' | 'Female' | null;
	parent_b_gender?: 'Male' | 'Female' | null;
}

export interface DirectChildResponse {
	/** The headline answer (first of `results`). */
	result: DirectResultItem | null;
	/** Every outcome for the pair. Longer than one only for gender-gated combos. */
	results?: DirectResultItem[];
}

export interface DirectPartnersResponse {
	partners: DirectResultItem[];
}

export interface DirectParentsResponse {
	parents: DirectResultItem[];
}

export interface BreedingStep {
	parent_a: string;
	parent_b: string;
	child: string;
	inherited_passives: string[];
	gender_feasible: boolean;
	/** Lineage refs: which occurrence of each parent tribe (a species can be
	 *  both a bred node and a source leaf in one chain). `*_step` indexes
	 *  `Chain.steps`, `*_source` indexes `Chain.sources`; one is set per parent. */
	parent_a_step?: number;
	parent_b_step?: number;
	parent_a_source?: number;
	parent_b_source?: number;
}

export interface ChainSource {
	type: 'owned' | 'selected' | 'wild';
	pal: string;
	display: string;
	gender: string;
	passives: string[];
	instance_id?: string;
	nickname?: string;
	level?: number;
	owner_uid?: string;
	raw_character_id?: string;
	[key: string]: unknown;
}

export interface Chain {
	target: string;
	generations: number;
	steps: BreedingStep[];
	final_passives: string[];
	sources: ChainSource[];
	gender_feasible: boolean;
	matched_passives: string[];
}

export interface ChainResponse {
	chains: Chain[];
	total: number;
	elapsed_ms: number;
	warnings: string[];
}

export interface SelectedPal {
	species: string;
	gender?: string | null;
	passives?: string[];
}

/**
 * Player-summary shape for the owner-search dropdown. PSP's core
 * `PlayerSummary` uses `nickname` (not `name`) and has no `guild_name`; the
 * page maps `nickname → name` and resolves `guild_name` from guild summaries.
 * This alias documents the fields the breeding UI consumes.
 */
export interface PlayerSummaryT {
	uid: string;
	name: string;
	pal_count: number;
	guild_name?: string;
}
export interface PalInput {
	character_id: string;
	gender?: string | null;
	passive_skills?: string[];
	origin: 'owned' | 'selected';
	instance_id?: string;
	nickname?: string;
	level?: number;
	owner_uid?: string;
}

export interface ChainRequest {
	target_pal: string;
	required_passives?: string[];
	target_gender?: string | null;
	max_generations?: number;
	max_results?: number;
	pals?: PalInput[];
	include_wild?: boolean;
}
