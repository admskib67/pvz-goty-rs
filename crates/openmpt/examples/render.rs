//! Renders a module to a 16-bit stereo WAV, optionally from an order.
//!
//!     cargo run -p openmpt --example render -- <module> <out.wav> [seconds] [order]

use std::io::Write;

const RATE: i32 = 44_100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: render <module> <out.wav> [seconds] [order]";
    let input = args.next().ok_or(usage)?;
    let output = args.next().ok_or(usage)?;
    let seconds: usize = args.next().map_or(Ok(20), |s| s.parse())?;
    let order: Option<i32> = args.next().map(|s| s.parse()).transpose()?;

    let mut module = openmpt::Module::from_memory(&std::fs::read(&input)?)?;
    println!(
        "libopenmpt {:?}: {} channels, {} orders",
        openmpt::library_version(),
        module.num_channels(),
        module.num_orders()
    );
    if let Some(order) = order {
        module.set_position_order_row(order, 0);
    }

    let mut samples = vec![0f32; RATE as usize * 2 * seconds];
    let frames = module.read_stereo(RATE, &mut samples);
    samples.truncate(frames * 2);
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    println!("rendered {frames} frames, peak {peak:.3}");

    let data: Vec<u8> =
        samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes()).collect();
    let mut f = std::fs::File::create(&output)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data.len() as u32).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    for v in [16u32, 1 | (2 << 16), RATE as u32, RATE as u32 * 4, 4 | (16 << 16)] {
        f.write_all(&v.to_le_bytes())?;
    }
    f.write_all(b"data")?;
    f.write_all(&(data.len() as u32).to_le_bytes())?;
    f.write_all(&data)?;
    Ok(())
}
