fn main() {
    println!("cargo:rerun-if-changed=../java-bridge/dist/sql-archive-bridge.jar");
    tauri_build::build()
}
