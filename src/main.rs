// Release builds are a GUI app: no console window. Debug keeps it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> iced::Result {
    kanvibe::startup::run()
}
