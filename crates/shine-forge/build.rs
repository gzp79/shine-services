use std::io::Result;

fn main() -> Result<()> {
    // Tell Cargo to rerun this build script if the proto files change
    println!("cargo:rerun-if-changed=map/proto/");

    prost_build::compile_protos(&["src/map/proto/*.proto"], &["src/map/proto/"])?;

    Ok(())
}
