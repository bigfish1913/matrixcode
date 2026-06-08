use tauri::Manager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Application state shared across all windows
pub struct AppState {
    // TODO: Add SessionManager, ProjectManager, AgentManager from matrixcode-core
    sessions: Arc<Mutex<Vec<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet(name: &str) -> Result<String, String> {
    Ok(format!("Hello, {}! Welcome to MatrixCode GUI.", name))
}

/// Create a new session
#[tauri::command]
async fn create_session(
    project_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let session_id = format!("session-{}", uuid::Uuid::new_v4());
    
    // Store session in state
    let mut sessions = state.sessions.lock().await;
    sessions.push(session_id.clone());
    
    Ok(session_id)
}

/// Get all sessions
#[tauri::command]
async fn get_sessions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let sessions = state.sessions.lock().await;
    Ok(sessions.clone())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            create_session,
            get_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}