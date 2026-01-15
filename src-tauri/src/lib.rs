// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
//
// Opcode 2.0 "Apex" - The World's Greatest Claude Code Wrapper
// Powered by Claude Opus 4.5

// Declare modules
pub mod checkpoint;
pub mod claude_binary;
pub mod commands;
pub mod process;
pub mod web_server;
// Note: Opcode 2.0 modules temporarily disabled pending module structure refactoring
// pub mod mcp;
// pub mod session;
// pub mod skills;
// pub mod tasks;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
