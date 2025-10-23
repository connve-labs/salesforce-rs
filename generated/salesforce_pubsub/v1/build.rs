fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .out_dir("src")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&["proto/pubsub_api.proto"], &["proto"])?;
    Ok(())
}
