const COMMANDS: &[&str] = &[
    "upload_plugin_chunk",
    "cancel_plugin_stream",
    "init_plugin_stream",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
