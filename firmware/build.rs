fn main() {
    println!("cargo:rustc-check-cfg=cfg(esp_idf_nvs_encryption)");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("espidf") {
        embuild::espidf::sysenv::output();
    }
}
