fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile(
        &[
            "../proto/program_service.proto",
            "../proto/video_storage_service.proto",
            "../proto/persistence.proto",
        ],
        &["../proto"],
    )?;
    Ok(())
}
