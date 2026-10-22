//! Lists or extracts a pak.
//!
//!     cargo run -p pak --example extract -- <main.pak>            # list
//!     cargo run -p pak --example extract -- <main.pak> <out_dir>  # extract

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let pak_path = PathBuf::from(args.next().ok_or("usage: extract <main.pak> [out_dir]")?);
    let out_dir = args.next().map(PathBuf::from);

    let pak = pak::Pak::open(&pak_path)?;
    match out_dir {
        None => {
            for e in pak.entries() {
                println!("{:>10}  {}", e.size, e.name);
            }
            eprintln!("{} files", pak.entries().len());
        }
        Some(out) => {
            for e in pak.entries() {
                let dest = out.join(&e.name);
                std::fs::create_dir_all(dest.parent().unwrap())?;
                std::fs::write(&dest, pak.get(&e.name).unwrap())?;
            }
            eprintln!("extracted {} files to {}", pak.entries().len(), out.display());
        }
    }
    Ok(())
}
