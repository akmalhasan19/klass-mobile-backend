fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Include both the local `proto/` tree and the system well-known types
    // (google/protobuf/*.proto), which live under /usr/include when
    // libprotobuf-dev is installed (Debian/Ubuntu / Docker images).
    let mut includes = vec!["proto".to_string()];
    for candidate in ["/usr/include", "/usr/local/include"] {
        if std::path::Path::new(candidate).join("google/protobuf/timestamp.proto").exists() {
            includes.push(candidate.to_string());
            break;
        }
    }

    tonic_build::configure()
        .build_server(true)
        .compile_protos(
            &["proto/klass/media/v1/media_generation.proto"],
            &includes.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;
    Ok(())
}
