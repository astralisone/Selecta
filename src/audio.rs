use anyhow::{anyhow, Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Tag};
use symphonia::core::probe::Hint;

use crate::library::FileTags;

pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Embedded tags (ID3, Vorbis comments, MP4 atoms, RIFF INFO).
    pub tags: FileTags,
}

/// Copy recognised tags into `out`. Earlier tags win, so call this with the
/// container-level metadata (e.g. an MP3's leading ID3v2) first.
fn collect_tags(out: &mut FileTags, tags: &[Tag]) {
    fn set(slot: &mut Option<String>, tag: &Tag) {
        if slot.is_none() {
            let v = tag.value.to_string();
            // RIFF INFO strings keep their NUL terminator.
            let v = v.trim_matches(|c: char| c.is_whitespace() || c == '\0');
            if !v.is_empty() {
                *slot = Some(v.to_string());
            }
        }
    }
    for tag in tags {
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) => set(&mut out.title, tag),
            Some(StandardTagKey::Artist) => set(&mut out.artist, tag),
            Some(StandardTagKey::Genre) => set(&mut out.genre, tag),
            Some(StandardTagKey::Label) => set(&mut out.label, tag),
            Some(StandardTagKey::Bpm) => {
                if out.bpm.is_none() {
                    out.bpm = tag
                        .value
                        .to_string()
                        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
                        .parse::<f32>()
                        .ok()
                        .filter(|b| *b > 0.0);
                }
            }
            _ => {
                // Symphonia has no standard key for musical key. ID3 uses
                // TKEY, Vorbis comments INITIALKEY / KEY, MP4 a freeform
                // `----:com.apple.iTunes:initialkey` atom.
                let k = tag.key.to_ascii_lowercase();
                if k == "tkey" || k == "key" || k.ends_with("initialkey") {
                    set(&mut out.key, tag);
                }
            }
        }
    }
}

/// Decode a whole audio file to mono f32 PCM.
///
/// For feature extraction we don't need stereo — downmixing halves the
/// work downstream. For very long tracks (DJ tools, ambient sets) this
/// can eat memory; the sampling strategy in features.rs compensates.
pub fn decode_mono(path: &Path, target_sr: u32) -> Result<DecodedAudio> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probing audio format")?;

    let mut tags = FileTags::default();
    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        collect_tags(&mut tags, rev.tags());
    }
    let mut format = probed.format;
    if let Some(rev) = format.metadata().current() {
        collect_tags(&mut tags, rev.tags());
    }
    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("no default track"))?;
    let track_id = track.id;
    let source_sr = track
        .codec_params
        .sample_rate
        .ok_or_else(|| anyhow!("unknown sample rate"))?;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("creating decoder")?;

    let mut raw_mono: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break, // EOF
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buf.copy_interleaved_ref(decoded);
                let interleaved = buf.samples();
                // Downmix to mono by averaging channels
                for frame in interleaved.chunks(channels) {
                    let sum: f32 = frame.iter().sum();
                    raw_mono.push(sum / channels as f32);
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    // Cheap linear-interp downsample. For MIR features this is fine;
    // if you want publication-quality resampling, swap in `rubato`.
    let samples = if source_sr == target_sr {
        raw_mono
    } else {
        resample_linear(&raw_mono, source_sr, target_sr)
    };

    Ok(DecodedAudio {
        samples,
        sample_rate: target_sr,
        tags,
    })
}

fn resample_linear(input: &[f32], from_sr: u32, to_sr: u32) -> Vec<f32> {
    let ratio = from_sr as f64 / to_sr as f64;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * ratio;
        let idx = src as usize;
        let frac = (src - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}
