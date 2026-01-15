// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Desktop entry point for Opcode 2.0 "Apex"
///
/// This is a thin wrapper that delegates to the library crate.
/// All application code, modules, and Tauri setup lives in lib.rs.
fn main() {
    opcode_lib::run();
}
