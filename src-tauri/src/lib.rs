// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//
// Opcode 2.0 "Apex" - The World's Greatest Claude Code Wrapper
// Powered by Claude Opus 4.5

// Declare modules
pub mod checkpoint;
pub mod claude_binary;
pub mod commands;
pub mod mcp;         // MCP Streamable HTTP transport for remote servers
pub mod process;
pub mod session;     // Session management with DashMap
pub mod skills;      // Unified skills system (slash commands, hooks, workflows)
pub mod tasks;       // Parallel tasks and background job management
pub mod web_server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
