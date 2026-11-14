//! `ts-bindings`: the TypeScript postcard (de)serializers of the stream states and commands, and the limits of the states.

use anyhow::Context;
use clap::Args as ClapArgs;
use postcard_bindgen::{
    generate_bindings,
    javascript::{build_package, GenerationSettings},
    PackageInfo,
};
mod twins;

use self::twins::{ChatCommand, ChatMessage, ChatState, PixelMapCommand, PixelMapState};
use shine_game::replica::{CHAT_MAX_TEXT_BYTES, CHAT_WINDOW, PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH};
use std::path::PathBuf;

/// Directory, below the output folder, of the generated postcard (de)serializer package.
const PACKAGE_NAME: &str = "postcard-bindings";
/// File, in the output folder, of the limits of the states that are not part of the postcard schema.
const CONSTANTS_FILE: &str = "snapshots.ts";

#[derive(ClapArgs)]
pub struct Args {
    /// The folder to generate the code into. It is created if missing; the files in it are overwritten.
    #[arg(short, long)]
    pub out: PathBuf,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.out).with_context(|| format!("cannot create {}", args.out.display()))?;

    build_package(
        &args.out,
        PackageInfo {
            name: PACKAGE_NAME.into(),
            version: "0.1.0".try_into().expect("a valid version"),
        },
        // Flat types: the module path of the twins is an implementation detail, and a namespace that only
        // holds other namespaces makes the generator panic.
        GenerationSettings::enable_all().module_structure(false),
        generate_bindings!(ChatMessage, ChatState, ChatCommand, PixelMapState, PixelMapCommand),
    )
    .with_context(|| format!("cannot generate the package into {}", args.out.display()))?;

    let constants = args.out.join(CONSTANTS_FILE);
    std::fs::write(&constants, constants_source()).with_context(|| format!("cannot write {}", constants.display()))?;

    println!(
        "Generated {} and {}/ into {}",
        CONSTANTS_FILE,
        PACKAGE_NAME,
        args.out.display()
    );
    Ok(())
}

fn constants_source() -> String {
    CONSTANTS_TEMPLATE
        .replace("{{CHAT_WINDOW}}", &CHAT_WINDOW.to_string())
        .replace("{{CHAT_MAX_TEXT_BYTES}}", &CHAT_MAX_TEXT_BYTES.to_string())
        .replace("{{PIXEL_MAP_WIDTH}}", &PIXEL_MAP_WIDTH.to_string())
        .replace("{{PIXEL_MAP_HEIGHT}}", &PIXEL_MAP_HEIGHT.to_string())
        .replace("{{PIXEL_MAP_COLORS}}", &PIXEL_MAP_COLORS.to_string())
}

const CONSTANTS_TEMPLATE: &str = include_str!("snapshots.ts.tmpl");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_have_no_unresolved_placeholder() {
        assert!(!constants_source().contains("{{"));
    }
}
