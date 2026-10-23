//! Render every factory preset to a WAV file (soft hit, then hard hit).
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example render -- out_dir
//! ```

use stabkussion_dsp::{Engine, presets::PRESETS};

const SR: u32 = 48_000;

mod common;
use common::write_wav;

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "renders".into()));
    std::fs::create_dir_all(&dir)?;
    for preset in PRESETS {
        let mut engine = Engine::new(SR as f32);
        engine.load_patch(0, &preset.patch());
        engine.snap_params();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for velocity in [0.35, 1.0] {
            engine.trigger(0, velocity);
            let (l, r) = engine.render_vec(SR as usize * 2);
            left.extend(l);
            right.extend(r);
        }
        let peak = left.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = (left.iter().map(|s| s * s).sum::<f32>() / left.len() as f32).sqrt();
        let name: String = preset
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let path = dir.join(format!("{name}.wav"));
        write_wav(&path, SR, &left, &right)?;
        println!(
            "{:<20} peak {:>6.3}  rms {:>6.3}  -> {}",
            preset.name,
            peak,
            rms,
            path.display()
        );
    }
    Ok(())
}
