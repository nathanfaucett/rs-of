fn main() {
    println!("cargo:rustc-cfg=device_service_ofdb");
    println!("cargo:warning=Building device-service with ofdb feature check");
}
