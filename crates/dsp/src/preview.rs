//! Offline render of a single hit, for the panel's waveform display.

use crate::params::{ALL_PARAMS, VoicePatch};
use crate::voice::Voice;

/// Min/max envelope of one hit of `patch`, in `columns` equal slices of
/// `seconds`. Rendered at `fs` without oversampling or reverb: quick and
/// close enough to draw. Deterministic for a given patch.
pub fn hit_outline(
    patch: &VoicePatch,
    velocity: f32,
    fs: f32,
    seconds: f32,
    columns: usize,
) -> Vec<(f32, f32)> {
    let mut v = Voice::new(fs, 0x0bad_5eed);
    for p in ALL_PARAMS {
        v.set_param(p, patch[p.index()]);
    }
    v.snap_params();
    v.trigger(velocity);
    let total = (fs * seconds) as usize;
    let per = (total / columns.max(1)).max(1);
    (0..columns)
        .map(|_| {
            (0..per).fold((0.0f32, 0.0f32), |(lo, hi), _| {
                let s = v.tick();
                (lo.min(s), hi.max(s))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::default_patch;

    #[test]
    fn outline_has_the_requested_shape_and_decays() {
        let o = hit_outline(&default_patch(), 0.8, 24_000.0, 1.0, 100);
        assert_eq!(o.len(), 100);
        let amp = |(lo, hi): (f32, f32)| hi - lo;
        assert!(amp(o[1]) > amp(o[99]) * 5.0);
        assert_eq!(o, hit_outline(&default_patch(), 0.8, 24_000.0, 1.0, 100));
    }
}
