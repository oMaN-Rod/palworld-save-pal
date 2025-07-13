use crate::graphql::AppSchema;
use crate::gvas_parser;
use crate::state::AppState;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn load_save_file(
    path: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("Received path from frontend: {}", path);

    match gvas_parser::parse_save_file(&path) {
        Ok(players) => {
            let mut app_state = state.0.lock().unwrap();
            app_state.players = Some(players);
            // Now that `Emitter` is in scope, this line will compile correctly.
            app_handle.emit("save-loaded", ()).unwrap();
            Ok(())
        }
        Err(e) => Err(format!("Failed to parse save file: {}", e)),
    }
}

#[tauri::command]
pub async fn graphql(
    schema: State<'_, AppSchema>,
    query: String,
    operation_name: Option<String>,
    variables: Option<Value>,
) -> Result<async_graphql::Response, ()> {
    // Note: The Result wrapper is still needed.
    let mut request = async_graphql::Request::new(query);
    if let Some(op_name) = operation_name {
        request = request.operation_name(op_name);
    }
    if let Some(vars) = variables {
        if let Ok(vars_map) = serde_json::from_value::<async_graphql::Variables>(vars) {
            request = request.variables(vars_map);
        }
    }
    Ok(schema.execute(request).await)
}
