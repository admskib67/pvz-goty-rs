//! Builds libopenmpt as static libraries for this machine, so the game can
//! link it without anyone installing it.
//!
//!     cargo run -p build-libopenmpt
//!
//! Downloads the pinned libopenmpt source release (checked against its
//! SHA-256), compiles it the way its Makefile does, and writes the libraries
//! plus libopenmpt's licence to `lib/libopenmpt/libopenmpt-<os>/<target>/`.
//! Run it once on each OS and commit the results.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const VERSION: &str = "0.8.9";
const URL: &str = "https://lib.openmpt.org/files/libopenmpt/src/libopenmpt-0.8.9+release.makefile.tar.gz";
const SHA256: &str = "9273b88b67973cc69e54d748ab1b749399d6d07695f1c37d0c59f88b4106074f";

const TARGET: &str = env!("BUILD_TARGET");

/// Folder for this OS, as named in `lib/libopenmpt/`.
fn os_dir() -> &'static str {
    if cfg!(target_os = "windows") {
        "libopenmpt-win"
    } else if cfg!(target_os = "macos") {
        "libopenmpt-darwin"
    } else if cfg!(target_os = "linux") {
        "libopenmpt-linux"
    } else {
        panic!("unsupported OS")
    }
}

fn sources(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    files.sort();
    files
}

fn download() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    eprintln!("downloading {URL}");
    let mut bytes = Vec::new();
    ureq::get(URL).call()?.into_body().into_reader().read_to_end(&mut bytes)?;
    let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if hash != SHA256 {
        return Err(format!("checksum mismatch: expected {SHA256}, got {hash}").into());
    }
    Ok(bytes)
}

fn configure(build: &mut cc::Build, root: &Path, out: &Path) {
    build
        .target(TARGET)
        .host(TARGET)
        .opt_level(3)
        .debug(false)
        .out_dir(out)
        .cargo_metadata(false)
        .warnings(false)
        .includes([root.join("src"), root.join("common"), root.to_path_buf(), root.join("include")]);
    for define in [
        "LIBOPENMPT_BUILD",
        "MPT_WITH_MINIZ",
        "MPT_WITH_MINIMP3",
        "MPT_WITH_STBVORBIS",
        "STB_VORBIS_NO_PULLDATA_API",
        "STB_VORBIS_NO_STDIO",
    ] {
        build.define(define, None);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Not `canonicalize()`: on Windows it yields `\\?\C:\...` paths, which cl.exe can't open.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).expect("repo root").to_path_buf();
    let libopenmpt = repo.join("lib").join("libopenmpt");
    let out = libopenmpt.join(os_dir()).join(TARGET);
    let work = repo.join("target").join("build-libopenmpt");

    let tarball = download()?;
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    tar::Archive::new(flate2::read::GzDecoder::new(&tarball[..])).unpack(&work)?;
    let root = work.join(format!("libopenmpt-{VERSION}+release"));

    std::fs::create_dir_all(&out)?;
    eprintln!("compiling for {TARGET} (this takes a minute)");

    // Bundled MP3, Vorbis and zlib decoders, which MO3 files need.
    let mut c = cc::Build::new();
    configure(&mut c, &root, &out);
    for f in ["minimp3/minimp3.c", "miniz/miniz.c", "stb_vorbis/stb_vorbis.c"] {
        c.file(root.join("include").join(f));
    }
    c.try_compile("openmpt_codecs")?;

    let mut cpp = cc::Build::new();
    configure(&mut cpp, &root, &out);
    cpp.cpp(true).std("c++17");
    if cpp.get_compiler().is_like_msvc() {
        cpp.flag("/EHsc").flag("/utf-8").flag("/bigobj");
    }
    for dir in [
        "src/openmpt/all",
        "src/openmpt/base",
        "src/openmpt/logging",
        "src/openmpt/random",
        "src/openmpt/fileformat_base",
        "src/openmpt/soundbase",
        "src/openmpt/soundfile_data",
        "common",
        "soundlib",
        "soundlib/plugins",
        "soundlib/plugins/dmo",
        "sounddsp",
        "libopenmpt",
    ] {
        cpp.files(sources(&root.join(dir), "cpp"));
    }
    cpp.try_compile("openmpt")?;

    // cc leaves object files beside the archives; keep only the libraries.
    for entry in std::fs::read_dir(&out)? {
        let path = entry?.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else if path.extension().is_some_and(|e| e == "o" || e == "obj") {
            std::fs::remove_file(&path)?;
        }
    }
    // BSD-3-Clause: the notice must accompany redistributed binaries.
    std::fs::copy(root.join("LICENSE"), out.join("LICENSE"))?;
    std::fs::write(out.join("VERSION"), format!("libopenmpt {VERSION}\n{URL}\nsha256 {SHA256}\n"))?;

    eprintln!("wrote {}", out.display());
    for entry in std::fs::read_dir(&out)? {
        let entry = entry?;
        eprintln!("  {:>10}  {}", entry.metadata()?.len(), entry.file_name().to_string_lossy());
    }
    Ok(())
}
