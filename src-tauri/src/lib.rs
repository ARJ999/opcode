// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//
// Opcode 2.0 "Apex" - The World's Greatest Claude Code Wrapper
// Powered by Claude Opus 4.5

// Declare ALL modules in the library crate
// This is the single source of truth for module declarations
pub mod checkpoint;
pub mod claude_binary;
pub mod commands;
pub mod mcp;         // MCP Streamable HTTP transport for remote servers
pub mod process;
pub mod session;     // Session management with DashMap
pub mod skills;      // Unified skills system (slash commands, hooks, workflows)
pub mod tasks;       // Parallel tasks and background job management
pub mod web_server;

// Re-export commonly used types
pub use checkpoint::state::CheckpointState;
pub use commands::agents::{AgentDb, init_database};
pub use commands::claude::ClaudeProcessState;
pub use commands::tasks::TaskManagerState;
pub use process::ProcessRegistryState;

use std::sync::Mutex;
use tauri::Manager;

#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

/// Main entry point for the Tauri application
/// This is called by both desktop (main.rs) and mobile builds
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize agents database
            let conn = init_database(&app.handle()).expect("Failed to initialize agents database");

            // Load and apply proxy settings from the database
            {
                let db = AgentDb(Mutex::new(conn));
                let proxy_settings = match db.0.lock() {
                    Ok(conn) => {
                        // Directly query proxy settings from the database
                        let mut settings = commands::proxy::ProxySettings::default();

                        let keys = vec![
                            ("proxy_enabled", "enabled"),
                            ("proxy_http", "http_proxy"),
                            ("proxy_https", "https_proxy"),
                            ("proxy_no", "no_proxy"),
                            ("proxy_all", "all_proxy"),
                        ];

                        for (db_key, field) in keys {
                            if let Ok(value) = conn.query_row(
                                "SELECT value FROM app_settings WHERE key = ?1",
                                rusqlite::params![db_key],
                                |row| row.get::<_, String>(0),
                            ) {
                                match field {
                                    "enabled" => settings.enabled = value == "true",
                                    "http_proxy" => {
                                        settings.http_proxy = Some(value).filter(|s| !s.is_empty())
                                    }
                                    "https_proxy" => {
                                        settings.https_proxy = Some(value).filter(|s| !s.is_empty())
                                    }
                                    "no_proxy" => {
                                        settings.no_proxy = Some(value).filter(|s| !s.is_empty())
                                    }
                                    "all_proxy" => {
                                        settings.all_proxy = Some(value).filter(|s| !s.is_empty())
                                    }
                                    _ => {}
                                }
                            }
                        }

                        log::info!("Loaded proxy settings: enabled={}", settings.enabled);
                        settings
                    }
                    Err(e) => {
                        log::warn!("Failed to lock database for proxy settings: {}", e);
                        commands::proxy::ProxySettings::default()
                    }
                };

                // Apply the proxy settings
                commands::proxy::apply_proxy_settings(&proxy_settings);
            }

            // Re-open the connection for the app to manage
            let conn = init_database(&app.handle()).expect("Failed to initialize agents database");
            app.manage(AgentDb(Mutex::new(conn)));

            // Initialize checkpoint state
            let checkpoint_state = CheckpointState::new();

            // Set the Claude directory path
            if let Ok(claude_dir) = dirs::home_dir()
                .ok_or_else(|| "Could not find home directory")
                .and_then(|home| {
                    let claude_path = home.join(".claude");
                    claude_path
                        .canonicalize()
                        .map_err(|_| "Could not find ~/.claude directory")
                })
            {
                let state_clone = checkpoint_state.clone();
                tauri::async_runtime::spawn(async move {
                    state_clone.set_claude_dir(claude_dir).await;
                });
            }

            app.manage(checkpoint_state);

            // Initialize process registry
            app.manage(ProcessRegistryState::default());

            // Initialize Claude process state
            app.manage(ClaudeProcessState::default());

            // Initialize task manager (Opcode 2.0)
            app.manage(TaskManagerState::default());

            // Apply window vibrancy with rounded corners on macOS
            #[cfg(target_os = "macos")]
            {
                let window = app.get_webview_window("main").unwrap();

                // Try different vibrancy materials that support rounded corners
                let materials = [
                    NSVisualEffectMaterial::UnderWindowBackground,
                    NSVisualEffectMaterial::WindowBackground,
                    NSVisualEffectMaterial::Popover,
                    NSVisualEffectMaterial::Menu,
                    NSVisualEffectMaterial::Sidebar,
                ];

                let mut applied = false;
                for material in materials.iter() {
                    if apply_vibrancy(&window, *material, None, Some(12.0)).is_ok() {
                        applied = true;
                        break;
                    }
                }

                if !applied {
                    // Fallback without rounded corners
                    apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::WindowBackground,
                        None,
                        None,
                    )
                    .expect("Failed to apply any window vibrancy");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Claude & Project Management
            commands::claude::list_projects,
            commands::claude::create_project,
            commands::claude::get_project_sessions,
            commands::claude::get_home_directory,
            commands::claude::get_claude_settings,
            commands::claude::open_new_session,
            commands::claude::get_system_prompt,
            commands::claude::check_claude_version,
            commands::claude::save_system_prompt,
            commands::claude::save_claude_settings,
            commands::claude::find_claude_md_files,
            commands::claude::read_claude_md_file,
            commands::claude::save_claude_md_file,
            commands::claude::load_session_history,
            commands::claude::execute_claude_code,
            commands::claude::continue_claude_code,
            commands::claude::resume_claude_code,
            commands::claude::cancel_claude_execution,
            commands::claude::list_running_claude_sessions,
            commands::claude::get_claude_session_output,
            commands::claude::list_directory_contents,
            commands::claude::search_files,
            commands::claude::get_recently_modified_files,
            commands::claude::get_hooks_config,
            commands::claude::update_hooks_config,
            commands::claude::validate_hook_command,
            // Checkpoint Management
            commands::claude::create_checkpoint,
            commands::claude::restore_checkpoint,
            commands::claude::list_checkpoints,
            commands::claude::fork_from_checkpoint,
            commands::claude::get_session_timeline,
            commands::claude::update_checkpoint_settings,
            commands::claude::get_checkpoint_diff,
            commands::claude::track_checkpoint_message,
            commands::claude::track_session_messages,
            commands::claude::check_auto_checkpoint,
            commands::claude::cleanup_old_checkpoints,
            commands::claude::get_checkpoint_settings,
            commands::claude::clear_checkpoint_manager,
            commands::claude::get_checkpoint_state_stats,
            // Agent Management
            commands::agents::list_agents,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::delete_agent,
            commands::agents::get_agent,
            commands::agents::execute_agent,
            commands::agents::list_agent_runs,
            commands::agents::get_agent_run,
            commands::agents::list_agent_runs_with_metrics,
            commands::agents::get_agent_run_with_real_time_metrics,
            commands::agents::list_running_sessions,
            commands::agents::kill_agent_session,
            commands::agents::get_session_status,
            commands::agents::cleanup_finished_processes,
            commands::agents::get_session_output,
            commands::agents::get_live_session_output,
            commands::agents::stream_session_output,
            commands::agents::load_agent_session_history,
            commands::agents::get_claude_binary_path,
            commands::agents::set_claude_binary_path,
            commands::agents::list_claude_installations,
            commands::agents::export_agent,
            commands::agents::export_agent_to_file,
            commands::agents::import_agent,
            commands::agents::import_agent_from_file,
            commands::agents::fetch_github_agents,
            commands::agents::fetch_github_agent_content,
            commands::agents::import_agent_from_github,
            // Usage & Analytics
            commands::usage::get_usage_stats,
            commands::usage::get_usage_by_date_range,
            commands::usage::get_usage_details,
            commands::usage::get_session_stats,
            // MCP (Model Context Protocol)
            commands::mcp::mcp_add,
            commands::mcp::mcp_list,
            commands::mcp::mcp_get,
            commands::mcp::mcp_remove,
            commands::mcp::mcp_add_json,
            commands::mcp::mcp_add_from_claude_desktop,
            commands::mcp::mcp_serve,
            commands::mcp::mcp_test_connection,
            commands::mcp::mcp_reset_project_choices,
            commands::mcp::mcp_get_server_status,
            commands::mcp::mcp_read_project_config,
            commands::mcp::mcp_save_project_config,
            // Storage Management
            commands::storage::storage_list_tables,
            commands::storage::storage_read_table,
            commands::storage::storage_update_row,
            commands::storage::storage_delete_row,
            commands::storage::storage_insert_row,
            commands::storage::storage_execute_sql,
            commands::storage::storage_reset_database,
            // Slash Commands
            commands::slash_commands::slash_commands_list,
            commands::slash_commands::slash_command_get,
            commands::slash_commands::slash_command_save,
            commands::slash_commands::slash_command_delete,
            // Proxy Settings
            commands::proxy::get_proxy_settings,
            commands::proxy::save_proxy_settings,
            // Remote MCP Servers (Opcode 2.0)
            commands::remote_mcp::list_remote_mcp_servers,
            commands::remote_mcp::add_remote_mcp_server,
            commands::remote_mcp::remove_remote_mcp_server,
            commands::remote_mcp::test_remote_mcp_connection,
            commands::remote_mcp::list_remote_mcp_tools,
            commands::remote_mcp::call_remote_mcp_tool,
            commands::remote_mcp::update_remote_mcp_server,
            // Skills System (Opcode 2.0)
            commands::skills::list_skills,
            commands::skills::get_skill,
            commands::skills::create_slash_command,
            commands::skills::create_hook,
            commands::skills::update_skill,
            commands::skills::delete_skill,
            commands::skills::execute_slash_command,
            commands::skills::list_slash_commands,
            commands::skills::import_claude_code_skills,
            commands::skills::import_skill_from_github,
            // Parallel Tasks Manager (Opcode 2.0)
            commands::tasks::list_tasks,
            commands::tasks::list_active_tasks,
            commands::tasks::list_background_tasks,
            commands::tasks::get_task,
            commands::tasks::cancel_task,
            commands::tasks::clear_completed_tasks,
            commands::tasks::get_task_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
