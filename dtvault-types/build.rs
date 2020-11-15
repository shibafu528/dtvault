fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../proto/program_service.proto")?;
    tonic_build::compile_protos("../proto/video_storage_service.proto")?;
    Ok(())
}
