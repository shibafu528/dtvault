fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../proto");
    tonic_build::configure().compile(
        &[
            "../proto/program_service.proto",
            "../proto/video_storage_service.proto",
            "../proto/persistence.proto",
            "../proto/encoder_service.proto",
        ],
        &["../proto"],
    )?;
    Ok(())
}
