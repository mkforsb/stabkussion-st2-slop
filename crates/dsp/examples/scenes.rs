//! Render every cross-modulation scene to a WAV file: channel 1 hit hard,
//! then channel 2, then both together, two seconds apart.
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example scenes -- out_dir
//! ```

use stabkussion_dsp::{Engine, presets::SCENES};

const SR: u32 = 48_000;

mod common;
use common::write_wav;

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "renders".into()));
    std::fs::create_dir_all(&dir)?;
    for scene in SCENES {
        let mut engine = Engine::new(SR as f32);
        for (v, patch) in scene.patches().iter().enumerate() {
            engine.load_patch(v, patch);
        }
        engine.snap_params();
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for hits in [&[0usize][..], &[1], &[0, 1]] {
            for &v in hits {
                engine.trigger(v, 1.0);
            }
            let (l, r) = engine.render_vec(SR as usize * 2);
            left.extend(l);
            right.extend(r);
        }
        let peak = left
            .iter()
            .chain(&right)
            .fold(0.0f32, |m, s| m.max(s.abs()));
        let name: String = scene
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
        let path = dir.join(format!("scene_{name}.wav"));
        write_wav(&path, SR, &left, &right)?;
        println!(
            "{:<20} peak {:>6.1} dB, hits {} + {}  -> {}",
            scene.name,
            20.0 * peak.max(1e-9).log10(),
            engine.hit_count(0),
            engine.hit_count(1),
            path.display()
        );
    }
    Ok(())
}
