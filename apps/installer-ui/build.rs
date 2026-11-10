fn main() {
    println!("cargo:rerun-if-changed=installer-ui.rc");
    println!("cargo:rerun-if-changed=installer-ui.manifest");
    println!("cargo:rerun-if-changed=../app/icons/icon.ico");
    embed_resource::compile("installer-ui.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed the installer icon");
}
