fn main() {
    // `sqlx::migrate!` embute as migrações no binário: recompila quando mudarem.
    println!("cargo:rerun-if-changed=migrations");
    tauri_build::build()
}
