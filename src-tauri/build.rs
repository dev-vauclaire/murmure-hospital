fn main() {
    println!("cargo:rerun-if-changed=../resources/hospital/user-guide");
    tauri_build::build()
}
