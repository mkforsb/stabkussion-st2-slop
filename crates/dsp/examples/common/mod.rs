//! Helpers shared by the offline tools: WAV files and rendering hits.

#![allow(dead_code)]

use std::io::{Read, Write};

use stabkussion_dsp::{Engine, VoicePatch};

/// A stereo WAV file (16/24/32-bit PCM or 32-bit float; mono is duplicated).
pub struct Wav {
    pub fs: f32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

pub fn read_wav(path: &std::path::Path) -> std::io::Result<Wav> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("not a WAV file"));
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (mut fmt, mut channels, mut fs, mut bits) = (0u16, 0usize, 0u32, 0u16);
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(pos + 4) as usize;
        let body = pos + 8;
        if id == b"fmt " {
            fmt = u16_at(body);
            channels = u16_at(body + 2) as usize;
            fs = u32_at(body + 4);
            bits = u16_at(body + 14);
            if fmt == 0xFFFE {
                fmt = u16_at(body + 24);
            }
        } else if id == b"data" {
            let data = &bytes[body..(body + len).min(bytes.len())];
            let width = bits as usize / 8;
            let frames = data.len() / (width * channels);
            let sample = |i: usize| -> f32 {
                let b = &data[i * width..];
                match (fmt, bits) {
                    (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
                    (1, 24) => {
                        (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0
                    }
                    (1, 32) => {
                        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0
                    }
                    (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    _ => 0.0,
                }
            };
            let mut left = Vec::with_capacity(frames);
            let mut right = Vec::with_capacity(frames);
            for f in 0..frames {
                left.push(sample(f * channels));
                right.push(sample(f * channels + channels.min(2) - 1));
            }
            return Ok(Wav {
                fs: fs as f32,
                left,
                right,
            });
        }
        pos = body + len + (len & 1);
    }
    Err(bad("no data chunk"))
}

pub fn write_wav(
    path: &std::path::Path,
    fs: u32,
    left: &[f32],
    right: &[f32],
) -> std::io::Result<()> {
    let frames = left.len() as u32;
    let data_len = frames * 4;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&fs.to_le_bytes())?;
    f.write_all(&(fs * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for (l, r) in left.iter().zip(right) {
        for s in [l, r] {
            f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
    }
    f.flush()
}

/// Linear-interpolation resampler (good enough for analysis below 8 kHz
/// after the reference has been lowpassed by being a bassy stab).
pub fn resample(x: &[f32], from: f32, to: f32) -> Vec<f32> {
    if (from - to).abs() < 0.5 {
        return x.to_vec();
    }
    let n = (x.len() as f64 * to as f64 / from as f64) as usize;
    (0..n)
        .map(|i| {
            let p = i as f64 * from as f64 / to as f64;
            let j = p as usize;
            let f = (p - j as f64) as f32;
            let a = x.get(j).copied().unwrap_or(0.0);
            let b = x.get(j + 1).copied().unwrap_or(0.0);
            a + (b - a) * f
        })
        .collect()
}

/// One full hit of `patch` on channel 1 through the whole engine (chorus,
/// echo and reverb included) at output rate `fs`.
pub fn render_hit(
    patch: &VoicePatch,
    velocity: f32,
    fs: f32,
    seconds: f32,
) -> (Vec<f32>, Vec<f32>) {
    let mut e = Engine::new(fs);
    e.load_patch(0, patch);
    e.snap_params();
    e.trigger(0, velocity);
    e.render_vec((fs * seconds) as usize)
}
