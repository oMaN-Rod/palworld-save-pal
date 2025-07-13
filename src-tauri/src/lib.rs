mod commands;
mod graphql;
mod gvas_parser;
mod models;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize the shared state
    let state = AppState::default();

    // Build the GraphQL schema and provide it with access to the shared state
    let schema = graphql::build_schema(state.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Make the state and schema available to all commands
        .manage(state)
        .manage(schema)
        // Register the frontend-callable commands
        .invoke_handler(tauri::generate_handler![
            commands::load_save_file,
            commands::graphql,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
