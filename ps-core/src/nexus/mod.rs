//! Nexus Mods: nxm:// links, download file names, update selection, and the
//! request and response shapes of the v1 REST and v2 GraphQL APIs.
pub mod files;
pub mod graphql;
pub mod links;
pub mod rate_limit;
pub mod rest;

pub use files::{
    archive_file_name, installed_version_base, latest_file, update_state, FileCategory, ModFile,
    UpdateState, IOSTORE_SUFFIX,
};
pub use graphql::{
    mod_files_body, mod_files_from_data, search_body, GraphqlError, GraphqlResponse, ModSummary,
    ModsPage, SearchData, SearchParams, SearchSort, Uploader, GAME_ID, MAX_OFFSET, MAX_PAGE,
    MAX_UPDATE_BATCH,
};
pub use links::{parse_nxm, NxmError, NxmLink, GAME_DOMAIN};
pub use rate_limit::RateLimit;
pub use rest::{
    categories_from_game, error_message, parse_account, Account, Category, DownloadLink,
};
