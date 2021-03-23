use const_format::concatcp;

const PROTO_ROOT: &str = "../proto";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", PROTO_ROOT);
    tonic_build::configure().compile(
        &[
            concatcp!(PROTO_ROOT, "/shibafu528/dtvault/central/persistence.proto"),
            concatcp!(PROTO_ROOT, "/shibafu528/dtvault/central/program_service.proto"),
            concatcp!(PROTO_ROOT, "/shibafu528/dtvault/encoder/encoder_service.proto"),
            concatcp!(PROTO_ROOT, "/shibafu528/dtvault/storage/video_storage_service.proto"),
        ],
        &[PROTO_ROOT],
    )?;
    Ok(())
}
