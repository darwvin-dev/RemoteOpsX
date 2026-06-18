// Prevents an extra console window on Windows in release. No effect on Linux.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    remoteopsx_lib::run()
}
