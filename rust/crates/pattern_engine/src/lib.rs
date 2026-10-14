// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Pattern Engine
//!
//! Converts raw experience into the AI's internal representation.
//! The encoder is seed-derived — patterns are generated THROUGH DF,
//! not encrypted with DF after the fact.
//!
//! Starts crude. Evolves through experience. This IS the dialect.
//!
//! Supports multimodal encoding:
//! - Text: byte-level + word-level layers
//! - Image: color histogram + spatial grid + edge detection + texture layers
//! - Audio: Goertzel spectral + RMS envelope + zero-crossing rate + spectral flux
//!
//! All modalities produce the same 256-dim normalized Vec<f32>.
//! Cross-modal bonding emerges naturally from shared vector space.

use df_core::Seed;
use image::GenericImageView;
use hound::WavReader;
use serde::{Serialize, Deserialize};

/// Intermediate analysis results from image feature extraction.
/// Holds raw feature data before seed-derived vector encoding.
#[derive(Clone, Debug)]
pub struct ImageFeatures {
    /// Color histogram: 64 bins (RGB space quantized to 4x4x4)
    /// Each bin holds normalized frequency (0.0 to 1.0)
    pub color_histogram: Vec<f32>,
    /// Spatial grid: 8x8 = 64 cells
    /// Each cell holds luminance variance (texture measure)
    pub spatial_variance: Vec<f32>,
    /// Edge orientation histogram: 64 bins (0° to 360° quantized)
    /// Each bin holds normalized edge magnitude at that orientation
    pub edge_histogram: Vec<f32>,
    /// Multi-scale texture: 64 bins
    /// Luminance difference statistics at 4 scales x 16 directions
    pub texture_histogram: Vec<f32>,
    /// Global statistics
    pub mean_brightness: f32,
    pub contrast: f32,
    /// Image dimensions
    pub width: u32,
    pub height: u32,
}

/// Intermediate analysis results from audio feature extraction.
/// Holds raw feature data before seed-derived vector encoding.
#[derive(Clone, Debug)]
pub struct AudioFeatures {
    /// Goertzel spectral energy: 64 frequency bins spanning ~27Hz to ~8kHz
    /// Each bin holds normalized energy at that frequency.
    pub spectral_bins: Vec<f32>,
    /// RMS envelope: 64 time windows
    /// Each window holds the root-mean-square amplitude (volume over time).
    pub rms_envelope: Vec<f32>,
    /// Zero-crossing rate: 64 time windows
    /// Each window holds normalized ZCR (high = noisy/fricative, low = tonal).
    pub zcr_envelope: Vec<f32>,
    /// Spectral flux: 64 frame pairs
    /// Measures spectral change between consecutive frames (onset detection).
    pub spectral_flux: Vec<f32>,
    /// Global statistics
    pub mean_rms: f32,
    pub peak_rms: f32,
    /// Audio metadata
    pub sample_rate: u32,
    pub channels: u16,
    pub total_samples: usize,
    pub duration_secs: f32,
}

/// Intermediate analysis results from video feature extraction.
/// Built from N keyframe ImageFeatures plus temporal analysis.
#[derive(Clone, Debug)]
pub struct VideoFeatures {
    /// Per-keyframe image features
    pub keyframe_features: Vec<ImageFeatures>,
    /// Mean visual pattern across all keyframes (averaged 256-dim vector)
    pub mean_visual: Vec<f32>,
    /// Motion delta: magnitude of change between consecutive keyframe patterns
    pub motion_deltas: Vec<f32>,
    /// Temporal arc: how the overall brightness/contrast evolve over time
    /// 64 bins mapping the video's temporal progression
    pub temporal_arc: Vec<f32>,
    /// Scene variance: how much each keyframe differs from the mean
    pub scene_variance: Vec<f32>,
    /// Video metadata
    pub num_keyframes: usize,
    pub total_frames: usize,
    pub width: u32,
    pub height: u32,
}

/// The AI's perception system. Converts input to internal patterns.
/// The encoding matrix is seeded — two different seeds produce
/// two completely different internal languages.
#[derive(Clone, Serialize, Deserialize)]
pub struct PatternEngine {
    /// Dimensionality of pattern vectors
    pattern_size: usize,
    /// The seed this engine derives from
    seed: Seed,
    /// The encoding matrix (256 byte values x pattern_size)
    /// Initialized from seed, refined through learning
    encoder: Vec<Vec<f32>>,
    /// Learned refinements layered on top of base encoder
    refinements: Vec<Vec<f32>>,
    /// How many bytes this engine has processed
    total_bytes_seen: u64,
    /// How many distinct inputs it has encoded
    experiences: u64,
    /// How many images this engine has encoded
    #[serde(default)]
    images_encoded: u64,
    /// How many audio clips this engine has encoded
    #[serde(default)]
    audios_encoded: u64,
    /// How many videos this engine has encoded
    #[serde(default)]
    videos_encoded: u64,
}
impl PatternEngine {
    /// Create a new pattern engine from a seed.
    /// The seed determines the initial encoding — this is DF at the core.
    pub fn new(pattern_size: usize, seed: &Seed) -> Self {
        // Derive the encoding matrix from the seed
        // Each of 256 byte values gets a pattern_size vector
        let mut encoder = Vec::with_capacity(256);
        for i in 0u16..256 {
            let row_seed = seed.derive(&i.to_le_bytes());
            let bytes = row_seed.as_bytes();
            let mut row = Vec::with_capacity(pattern_size);
            for j in 0..pattern_size {
                // Derive each float from seed bytes
                let idx = j % 32;
                let val = (bytes[idx] as f32 / 255.0) * 0.2 - 0.1;
                row.push(val);
            }
            encoder.push(row);
        }

        let refinements = vec![vec![0.0f32; pattern_size]; 256];

        Self {
            pattern_size,
            seed: seed.clone(),
            encoder,
            refinements,
            total_bytes_seen: 0,
            experiences: 0,
            images_encoded: 0,
            audios_encoded: 0,
            videos_encoded: 0,
        }
    }
    /// Encode raw input into a pattern vector.
    /// This is perception — the AI "seeing" something.
    ///
    /// Two encoding layers:
    /// 1. Byte-level: each character contributes its seed-derived vector
    ///    (captures character distribution, positional structure)
    /// 2. Word-level: each whitespace-delimited word contributes a
    ///    seed-derived vector from the word's hash
    ///    (captures vocabulary — "dog" and "cat" get different vectors)
    ///
    /// Both layers are summed before normalization, so the pattern
    /// reflects both character texture and word identity.
    pub fn encode(&mut self, input: &str) -> Vec<f32> {
        let bytes = input.as_bytes();
        self.total_bytes_seen += bytes.len() as u64;
        self.experiences += 1;

        let mut pattern = vec![0.0f32; self.pattern_size];

        // Layer 1: Byte-level encoding (character texture)
        for (i, &b) in bytes.iter().enumerate() {
            let idx = b as usize;
            let position_weight = 1.0 / (1.0 + 0.01 * i as f32);

            for j in 0..self.pattern_size {
                pattern[j] += (self.encoder[idx][j] + self.refinements[idx][j])
                    * position_weight;
            }
        }
        // Layer 2: Word-level encoding (vocabulary identity)
        // Each word gets a deterministic vector derived from the engine's
        // seed + the word content. This means "dog" always maps to the
        // same vector for this brain, but a different brain (different seed)
        // would map "dog" differently. This IS the dialect.
        let words: Vec<&str> = input.split_whitespace().collect();
        for (pos, word) in words.iter().enumerate() {
            // Derive a seed from the word (case-insensitive)
            let word_seed = self.seed.derive(word.to_lowercase().as_bytes());
            let word_bytes = word_seed.as_bytes();

            // Position decay for word order (gentler than byte-level)
            let word_weight = 1.0 / (1.0 + 0.05 * pos as f32);

            for j in 0..self.pattern_size {
                // Use two bytes per dimension for more variation
                let idx1 = j % 32;
                let idx2 = (j + 17) % 32; // offset prime to avoid alignment
                let val = ((word_bytes[idx1] as f32 + word_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * word_weight;
            }
        }

        // Normalize to unit vector
        let norm: f32 = pattern.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut pattern {
                *x /= norm;
            }
        }

        pattern
    }
    /// Encode raw image bytes into a pattern vector.
    /// This is visual perception — the AI "seeing" an image.
    ///
    /// Four encoding layers, each contributing to ALL 256 dims
    /// (same structure as text's byte + word layers — no partitioning):
    ///
    /// 1. Color histogram: 64 bins (4x4x4 RGB quantization).
    ///    Each bin derives a seed vector, scaled by frequency.
    /// 2. Spatial grid: 8x8 cells, each cell's luminance variance
    ///    drives a seed-derived contribution.
    /// 3. Edge detection: Sobel gradients binned into 64 orientation
    ///    bins, each bin derives a seed vector scaled by magnitude.
    /// 4. Multi-scale texture: luminance differences at 4 scales x
    ///    16 directions = 64 bins.
    ///
    /// All layers sum into one vector. Modality offset (~0.02) added
    /// via seed.derive(b"modality_image"). L2-normalized to unit length.
    ///
    /// Returns None if the image bytes can't be decoded.
    pub fn encode_image(&mut self, raw_bytes: &[u8]) -> Option<(Vec<f32>, ImageFeatures)> {
        // Decode the image
        let img = image::load_from_memory(raw_bytes).ok()?;
        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return None;
        }

        self.total_bytes_seen += raw_bytes.len() as u64;
        self.images_encoded += 1;

        // Extract all feature layers
        let features = self.extract_image_features(&img);

        // Build the pattern vector from features
        let pattern = self.features_to_pattern(&features);

        Some((pattern, features))
    }
    /// Extract image features into the intermediate ImageFeatures struct.
    fn extract_image_features(&self, img: &image::DynamicImage) -> ImageFeatures {
        let (width, height) = img.dimensions();
        let total_pixels = (width as f64 * height as f64).max(1.0);

        // ─── Layer 1: Color Histogram (64 bins) ────────────────
        // Quantize RGB to 4x4x4 cube (4 levels per channel)
        let mut color_histogram = vec![0.0f32; 64];
        let mut brightness_sum: f64 = 0.0;
        let mut brightness_sq_sum: f64 = 0.0;

        // Collect per-pixel data for all layers
        let rgb_img = img.to_rgb8();
        for pixel in rgb_img.pixels() {
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];

            // Quantize to 4 levels each (0-3)
            let rq = (r as usize >> 6).min(3);
            let gq = (g as usize >> 6).min(3);
            let bq = (b as usize >> 6).min(3);
            let bin = rq * 16 + gq * 4 + bq;
            color_histogram[bin] += 1.0;

            // Brightness (ITU-R BT.601 luminance)
            let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            brightness_sum += lum;
            brightness_sq_sum += lum * lum;
        }
        // Normalize color histogram to frequencies
        for bin in &mut color_histogram {
            *bin /= total_pixels as f32;
        }

        // Global brightness/contrast
        let mean_brightness = (brightness_sum / total_pixels) as f32 / 255.0;
        let variance = (brightness_sq_sum / total_pixels)
            - (brightness_sum / total_pixels).powi(2);
        let contrast = (variance.max(0.0).sqrt() / 255.0) as f32;

        // ─── Layer 2: Spatial Grid (8x8 = 64 cells) ───────────
        // Each cell: compute luminance variance (texture measure)
        let grid_w = 8usize;
        let grid_h = 8usize;
        let cell_w = (width as usize).max(grid_w) / grid_w;
        let cell_h = (height as usize).max(grid_h) / grid_h;
        let mut spatial_variance = vec![0.0f32; 64];

        let gray = img.to_luma8();
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let x0 = gx * cell_w;
                let y0 = gy * cell_h;
                let x1 = ((gx + 1) * cell_w).min(width as usize);
                let y1 = ((gy + 1) * cell_h).min(height as usize);

                let mut sum: f64 = 0.0;
                let mut sq_sum: f64 = 0.0;
                let mut count: f64 = 0.0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let lum = gray.get_pixel(x as u32, y as u32)[0] as f64;
                        sum += lum;
                        sq_sum += lum * lum;
                        count += 1.0;
                    }
                }

                let cell_idx = gy * grid_w + gx;
                if count > 0.0 {
                    let mean = sum / count;
                    let var = (sq_sum / count) - mean * mean;
                    // Normalize variance to 0..1 range (max possible ~16256)
                    spatial_variance[cell_idx] = (var.max(0.0).sqrt() / 128.0).min(1.0) as f32;
                }
            }
        }
        // ─── Layer 3: Edge Detection (64 orientation bins) ─────
        // Simple Sobel-like gradient: horizontal and vertical differences.
        // Bin by orientation (64 bins = ~5.6° each), weight by magnitude.
        let mut edge_histogram = vec![0.0f32; 64];
        let mut total_edge_mag: f64 = 0.0;

        for y in 1..(height.saturating_sub(1)) {
            for x in 1..(width.saturating_sub(1)) {
                // Sobel approximation using grayscale
                let tl = gray.get_pixel(x - 1, y - 1)[0] as f64;
                let tc = gray.get_pixel(x,     y - 1)[0] as f64;
                let tr = gray.get_pixel(x + 1, y - 1)[0] as f64;
                let ml = gray.get_pixel(x - 1, y    )[0] as f64;
                let mr = gray.get_pixel(x + 1, y    )[0] as f64;
                let bl = gray.get_pixel(x - 1, y + 1)[0] as f64;
                let bc = gray.get_pixel(x,     y + 1)[0] as f64;
                let br = gray.get_pixel(x + 1, y + 1)[0] as f64;

                // Sobel kernels
                let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
                let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

                let mag = (gx * gx + gy * gy).sqrt();
                if mag > 10.0 {
                    // atan2 gives angle in radians (-π to π), map to 0..64
                    let angle = gy.atan2(gx); // -π to π
                    let normalized = (angle + std::f64::consts::PI)
                        / (2.0 * std::f64::consts::PI); // 0.0 to 1.0
                    let bin = ((normalized * 64.0) as usize).min(63);
                    edge_histogram[bin] += mag as f32;
                    total_edge_mag += mag;
                }
            }
        }
        // Normalize edge histogram
        if total_edge_mag > 0.0 {
            for bin in &mut edge_histogram {
                *bin /= total_edge_mag as f32;
            }
        }

        // ─── Layer 4: Multi-Scale Texture (64 bins) ────────────
        // 4 scales x 16 directions = 64 bins.
        // At each scale, sample pixel pairs at that distance apart
        // in 16 evenly-spaced directions, accumulate absolute
        // luminance difference.
        let mut texture_histogram = vec![0.0f32; 64];
        let scales: [u32; 4] = [1, 2, 4, 8];

        for (scale_idx, &scale) in scales.iter().enumerate() {
            for dir_idx in 0u32..16 {
                let angle = (dir_idx as f64) * std::f64::consts::PI / 8.0;
                let dx = (angle.cos() * scale as f64).round() as i32;
                let dy = (angle.sin() * scale as f64).round() as i32;

                let mut diff_sum: f64 = 0.0;
                let mut count: f64 = 0.0;

                // Sample a grid of points (skip pixels for speed)
                let step = (scale as usize).max(2);
                let mut sy = 0u32;
                while sy < height {
                    let mut sx = 0u32;
                    while sx < width {
                        let nx = sx as i32 + dx;
                        let ny = sy as i32 + dy;
                        if nx >= 0 && nx < width as i32
                            && ny >= 0 && ny < height as i32
                        {
                            let a = gray.get_pixel(sx, sy)[0] as f64;
                            let b = gray.get_pixel(nx as u32, ny as u32)[0] as f64;
                            diff_sum += (a - b).abs();
                            count += 1.0;
                        }
                        sx += step as u32;
                    }
                    sy += step as u32;
                }

                let bin_idx = scale_idx * 16 + dir_idx as usize;
                if count > 0.0 {
                    // Normalize to 0..1 (max diff is 255)
                    texture_histogram[bin_idx] = ((diff_sum / count) / 255.0) as f32;
                }
            }
        }

        ImageFeatures {
            color_histogram,
            spatial_variance,
            edge_histogram,
            texture_histogram,
            mean_brightness,
            contrast,
            width,
            height,
        }
    }
    /// Convert extracted ImageFeatures into a 256-dim pattern vector.
    /// Each feature layer derives seed vectors that sum across ALL 256 dims
    /// (same approach as text's byte + word layers — no partitioning).
    fn features_to_pattern(&self, features: &ImageFeatures) -> Vec<f32> {
        let mut pattern = vec![0.0f32; self.pattern_size];

        // Derive the image-specific base seed
        let img_seed = self.seed.derive(b"image_encoder");

        // ─── Layer 1: Color histogram contribution ─────────────
        // Each of 64 color bins derives a full 256-dim seed vector,
        // weighted by that bin's frequency.
        let color_seed = img_seed.derive(b"color_histogram");
        for (bin_idx, &freq) in features.color_histogram.iter().enumerate() {
            if freq < 1e-6 { continue; } // skip empty bins
            let bin_seed = color_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 13) % 32;
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * freq;
            }
        }
        // ─── Layer 2: Spatial grid contribution ────────────────
        // Each of 64 grid cells derives a full 256-dim seed vector,
        // weighted by that cell's luminance variance.
        let spatial_seed = img_seed.derive(b"spatial_grid");
        for (cell_idx, &var) in features.spatial_variance.iter().enumerate() {
            if var < 1e-6 { continue; }
            let cell_seed = spatial_seed.derive(&(cell_idx as u16).to_le_bytes());
            let cell_bytes = cell_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 19) % 32; // different prime offset
                let val = ((cell_bytes[idx1] as f32 + cell_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * var;
            }
        }

        // ─── Layer 3: Edge orientation contribution ────────────
        // Each of 64 orientation bins derives a full 256-dim seed vector,
        // weighted by normalized edge magnitude at that angle.
        let edge_seed = img_seed.derive(b"edge_orientation");
        for (bin_idx, &mag) in features.edge_histogram.iter().enumerate() {
            if mag < 1e-6 { continue; }
            let bin_seed = edge_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 23) % 32; // another prime offset
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * mag;
            }
        }
        // ─── Layer 4: Multi-scale texture contribution ─────────
        // Each of 64 texture bins (4 scales x 16 directions) derives
        // a full 256-dim seed vector, weighted by mean difference.
        let texture_seed = img_seed.derive(b"texture_multiscale");
        for (bin_idx, &diff) in features.texture_histogram.iter().enumerate() {
            if diff < 1e-6 { continue; }
            let bin_seed = texture_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 29) % 32; // yet another prime offset
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * diff;
            }
        }

        // ─── Global statistics contribution ────────────────────
        // Brightness and contrast each get a seed-derived vector
        let bright_seed = img_seed.derive(b"brightness");
        let bright_bytes = bright_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let val = (bright_bytes[idx] as f32 / 255.0) * 0.1 - 0.05;
            pattern[j] += val * features.mean_brightness;
        }

        let contrast_seed = img_seed.derive(b"contrast");
        let contrast_bytes = contrast_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let val = (contrast_bytes[idx] as f32 / 255.0) * 0.1 - 0.05;
            pattern[j] += val * features.contrast;
        }
        // ─── Modality offset ───────────────────────────────────
        // Tiny bias so image vectors occupy a slightly different
        // region than text vectors. ~0.02 magnitude — small enough
        // that genuinely related cross-modal patterns still bond,
        // but large enough that random images don't false-match
        // random text.
        let modality_seed = self.seed.derive(b"modality_image");
        let mod_bytes = modality_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let offset = (mod_bytes[idx] as f32 / 255.0) * 0.04 - 0.02;
            pattern[j] += offset;
        }

        // ─── L2 Normalize to unit vector ───────────────────────
        let norm: f32 = pattern.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut pattern {
                *x /= norm;
            }
        }

        pattern
    }
    // ─── Audio Encoding ──────────────────────────────────

    /// Encode raw WAV audio bytes into a pattern vector.
    /// This is auditory perception — the AI "hearing" a sound.
    ///
    /// Four encoding layers, each contributing to ALL 256 dims:
    ///
    /// 1. Goertzel spectral: 64 frequency bins (~27Hz to ~8kHz).
    ///    Each bin derives a seed vector, scaled by energy.
    /// 2. RMS envelope: 64 time windows, amplitude over time.
    /// 3. Zero-crossing rate: 64 time windows (tonal vs noisy).
    /// 4. Spectral flux: 64 frame pairs (onset/change detection).
    ///
    /// All layers sum into one vector. Modality offset (~0.02) added
    /// via seed.derive(b"modality_audio"). L2-normalized to unit length.
    ///
    /// Returns None if the WAV bytes can't be decoded.
    pub fn encode_audio(&mut self, raw_bytes: &[u8]) -> Option<(Vec<f32>, AudioFeatures)> {
        let cursor = std::io::Cursor::new(raw_bytes);
        let reader = WavReader::new(cursor).ok()?;
        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = spec.channels;

        // Read all samples as f64, mix to mono
        let samples: Vec<f64> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let bits = spec.bits_per_sample;
                let max_val = (1u64 << (bits - 1)) as f64;
                reader.into_samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f64 / max_val)
                    .collect()
            }
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f64)
                    .collect()
            }
        };

        if samples.is_empty() {
            return None;
        }

        // Mix to mono if multi-channel
        let mono: Vec<f64> = if channels > 1 {
            samples.chunks(channels as usize)
                .map(|chunk| {
                    chunk.iter().sum::<f64>() / channels as f64
                })
                .collect()
        } else {
            samples
        };

        let total_samples = mono.len();
        let duration_secs = total_samples as f32 / sample_rate as f32;

        self.total_bytes_seen += raw_bytes.len() as u64;
        self.audios_encoded += 1;

        let features = self.extract_audio_features(&mono, sample_rate);

        let pattern = self.audio_features_to_pattern(&features);

        Some((pattern, AudioFeatures {
            spectral_bins: features.spectral_bins,
            rms_envelope: features.rms_envelope,
            zcr_envelope: features.zcr_envelope,
            spectral_flux: features.spectral_flux,
            mean_rms: features.mean_rms,
            peak_rms: features.peak_rms,
            sample_rate,
            channels,
            total_samples,
            duration_secs,
        }))
    }

    /// Extract audio features from mono f64 samples.
    fn extract_audio_features(&self, mono: &[f64], sample_rate: u32) -> AudioFeatures {
        let total_samples = mono.len();
        let num_bins = 64usize;

        // ─── Layer 1: Goertzel Spectral (64 frequency bins) ────
        // Logarithmically spaced from ~27Hz (A0) to ~8kHz.
        // Goertzel algorithm: O(N) per frequency, no FFT needed.
        let min_freq: f64 = 27.5;  // A0
        let max_freq: f64 = 8000.0;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        let mut spectral_bins = vec![0.0f32; num_bins];
        let mut max_energy: f64 = 0.0;

        // Use a window of samples for Goertzel (cap at 4096 for speed)
        let window_size = total_samples.min(4096);
        let window = &mono[..window_size];

        for bin in 0..num_bins {
            // Log-spaced frequency for this bin
            let freq = (log_min + (bin as f64 / (num_bins - 1) as f64) * (log_max - log_min)).exp();
            let energy = Self::goertzel(window, sample_rate as f64, freq);
            spectral_bins[bin] = energy as f32;
            if energy > max_energy {
                max_energy = energy;
            }
        }

        // Normalize spectral bins
        if max_energy > 0.0 {
            for bin in &mut spectral_bins {
                *bin /= max_energy as f32;
            }
        }

        // ─── Layer 2: RMS Envelope (64 time windows) ───────────
        // Divide audio into 64 equal windows, compute RMS per window.
        let window_len = (total_samples / num_bins).max(1);
        let mut rms_envelope = vec![0.0f32; num_bins];
        let mut mean_rms: f64 = 0.0;
        let mut peak_rms: f32 = 0.0;

        for w in 0..num_bins {
            let start = w * window_len;
            let end = ((w + 1) * window_len).min(total_samples);
            if start >= total_samples { break; }

            let mut sum_sq: f64 = 0.0;
            let count = (end - start) as f64;
            for &s in &mono[start..end] {
                sum_sq += s * s;
            }
            let rms = (sum_sq / count.max(1.0)).sqrt() as f32;
            rms_envelope[w] = rms;
            mean_rms += rms as f64;
            if rms > peak_rms { peak_rms = rms; }
        }

        mean_rms /= num_bins as f64;

        // Normalize RMS envelope by peak
        if peak_rms > 0.0 {
            for v in &mut rms_envelope {
                *v /= peak_rms;
            }
        }

        // ─── Layer 3: Zero-Crossing Rate (64 time windows) ─────
        // ZCR: how often the signal crosses zero per window.
        // High ZCR = noisy/fricative, Low ZCR = tonal/vowel.
        let mut zcr_envelope = vec![0.0f32; num_bins];
        let mut max_zcr: f32 = 0.0;

        for w in 0..num_bins {
            let start = w * window_len;
            let end = ((w + 1) * window_len).min(total_samples);
            if start >= total_samples || end <= start + 1 { break; }

            let mut crossings: usize = 0;
            for i in (start + 1)..end {
                if (mono[i] >= 0.0) != (mono[i - 1] >= 0.0) {
                    crossings += 1;
                }
            }
            let zcr = crossings as f32 / (end - start) as f32;
            zcr_envelope[w] = zcr;
            if zcr > max_zcr { max_zcr = zcr; }
        }

        // Normalize ZCR
        if max_zcr > 0.0 {
            for v in &mut zcr_envelope {
                *v /= max_zcr;
            }
        }

        // ─── Layer 4: Spectral Flux (64 frame pairs) ───────────
        // Measures spectral change between consecutive frames.
        // High flux = onset/transient, Low flux = sustained.
        // Use simplified approach: magnitude difference between
        // consecutive Goertzel snapshots of short frames.
        let frame_size = (total_samples / (num_bins + 1)).max(64);
        let mut spectral_flux = vec![0.0f32; num_bins];
        let mut max_flux: f32 = 0.0;

        // Only compute 8 representative Goertzel bins per frame for speed
        let flux_freqs: Vec<f64> = (0..8).map(|i| {
            (log_min + (i as f64 / 7.0) * (log_max - log_min)).exp()
        }).collect();

        let mut prev_spectrum = vec![0.0f64; 8];
        // Compute first frame spectrum
        if frame_size <= total_samples {
            let frame = &mono[..frame_size.min(total_samples)];
            for (fi, &freq) in flux_freqs.iter().enumerate() {
                prev_spectrum[fi] = Self::goertzel(frame, sample_rate as f64, freq);
            }
        }

        for w in 0..num_bins {
            let start = ((w + 1) * frame_size).min(total_samples);
            let end = (start + frame_size).min(total_samples);
            if start >= total_samples || end <= start { break; }

            let frame = &mono[start..end];
            let mut cur_spectrum = vec![0.0f64; 8];
            let mut flux: f64 = 0.0;

            for (fi, &freq) in flux_freqs.iter().enumerate() {
                cur_spectrum[fi] = Self::goertzel(frame, sample_rate as f64, freq);
                let diff = cur_spectrum[fi] - prev_spectrum[fi];
                // Half-wave rectification: only positive changes (onsets)
                if diff > 0.0 {
                    flux += diff;
                }
            }

            spectral_flux[w] = flux as f32;
            if flux as f32 > max_flux { max_flux = flux as f32; }
            prev_spectrum = cur_spectrum;
        }

        // Normalize spectral flux
        if max_flux > 0.0 {
            for v in &mut spectral_flux {
                *v /= max_flux;
            }
        }

        AudioFeatures {
            spectral_bins,
            rms_envelope,
            zcr_envelope,
            spectral_flux,
            mean_rms: mean_rms as f32,
            peak_rms,
            sample_rate,
            channels: 1,
            total_samples: mono.len(),
            duration_secs: mono.len() as f32 / sample_rate as f32,
        }
    }

    /// Goertzel algorithm: compute energy at a single frequency.
    /// O(N) per frequency — no FFT needed, no external crates.
    fn goertzel(samples: &[f64], sample_rate: f64, target_freq: f64) -> f64 {
        let n = samples.len();
        if n == 0 { return 0.0; }

        let k = (target_freq * n as f64 / sample_rate).round();
        let w = 2.0 * std::f64::consts::PI * k / n as f64;
        let coeff = 2.0 * w.cos();

        let mut s0: f64 = 0.0;
        let mut s1: f64 = 0.0;
        let mut s2: f64;

        for &sample in samples {
            s2 = s1;
            s1 = s0;
            s0 = sample + coeff * s1 - s2;
        }

        // Power at this frequency
        let power = s0 * s0 + s1 * s1 - coeff * s0 * s1;
        power.abs().sqrt() / n as f64
    }

    /// Convert extracted AudioFeatures into a 256-dim pattern vector.
    /// Each feature layer derives seed vectors that sum across ALL 256 dims.
    fn audio_features_to_pattern(&self, features: &AudioFeatures) -> Vec<f32> {
        let mut pattern = vec![0.0f32; self.pattern_size];

        let aud_seed = self.seed.derive(b"audio_encoder");

        // ─── Layer 1: Goertzel spectral contribution ───────────
        let spectral_seed = aud_seed.derive(b"goertzel_spectral");
        for (bin_idx, &energy) in features.spectral_bins.iter().enumerate() {
            if energy < 1e-6 { continue; }
            let bin_seed = spectral_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 11) % 32;
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * energy;
            }
        }

        // ─── Layer 2: RMS envelope contribution ────────────────
        let rms_seed = aud_seed.derive(b"rms_envelope");
        for (win_idx, &rms) in features.rms_envelope.iter().enumerate() {
            if rms < 1e-6 { continue; }
            let win_seed = rms_seed.derive(&(win_idx as u16).to_le_bytes());
            let win_bytes = win_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 17) % 32;
                let val = ((win_bytes[idx1] as f32 + win_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * rms;
            }
        }

        // ─── Layer 3: Zero-crossing rate contribution ──────────
        let zcr_seed = aud_seed.derive(b"zero_crossing_rate");
        for (win_idx, &zcr) in features.zcr_envelope.iter().enumerate() {
            if zcr < 1e-6 { continue; }
            let win_seed = zcr_seed.derive(&(win_idx as u16).to_le_bytes());
            let win_bytes = win_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 23) % 32;
                let val = ((win_bytes[idx1] as f32 + win_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * zcr;
            }
        }

        // ─── Layer 4: Spectral flux contribution ───────────────
        let flux_seed = aud_seed.derive(b"spectral_flux");
        for (pair_idx, &flux) in features.spectral_flux.iter().enumerate() {
            if flux < 1e-6 { continue; }
            let pair_seed = flux_seed.derive(&(pair_idx as u16).to_le_bytes());
            let pair_bytes = pair_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 29) % 32;
                let val = ((pair_bytes[idx1] as f32 + pair_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * flux;
            }
        }

        // ─── Global statistics contribution ────────────────────
        let mean_rms_seed = aud_seed.derive(b"mean_rms");
        let mean_bytes = mean_rms_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let val = (mean_bytes[idx] as f32 / 255.0) * 0.1 - 0.05;
            pattern[j] += val * features.mean_rms;
        }

        let peak_seed = aud_seed.derive(b"peak_rms");
        let peak_bytes = peak_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let val = (peak_bytes[idx] as f32 / 255.0) * 0.1 - 0.05;
            pattern[j] += val * features.peak_rms;
        }

        // ─── Modality offset ───────────────────────────────────
        let modality_seed = self.seed.derive(b"modality_audio");
        let mod_bytes = modality_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let offset = (mod_bytes[idx] as f32 / 255.0) * 0.04 - 0.02;
            pattern[j] += offset;
        }

        // ─── L2 Normalize to unit vector ───────────────────────
        let norm: f32 = pattern.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut pattern {
                *x /= norm;
            }
        }

        pattern
    }

    // ─── Video Encoding ──────────────────────────────────

    /// Encode a video as a sequence of image frames.
    /// Takes a Vec of raw image bytes (one per keyframe).
    /// Each keyframe is encoded via encode_image(), then temporal
    /// layers are computed across the sequence.
    ///
    /// Returns a single 256-dim pattern (the video's "identity")
    /// plus the full VideoFeatures for inspection.
    ///
    /// For short clips: all frames are keyframes.
    /// For long videos: caller should sample N evenly-spaced frames.
    pub fn encode_video(
        &mut self,
        keyframe_bytes: &[&[u8]],
    ) -> Option<(Vec<f32>, VideoFeatures)> {
        if keyframe_bytes.is_empty() {
            return None;
        }

        self.videos_encoded += 1;

        // Encode each keyframe through the image encoder
        let mut keyframe_patterns: Vec<Vec<f32>> = Vec::new();
        let mut keyframe_features: Vec<ImageFeatures> = Vec::new();

        for frame_bytes in keyframe_bytes {
            if let Some((pattern, features)) = self.encode_image(frame_bytes) {
                keyframe_patterns.push(pattern);
                keyframe_features.push(features);
            }
        }

        if keyframe_patterns.is_empty() {
            return None;
        }

        let n = keyframe_patterns.len();
        let (width, height) = if !keyframe_features.is_empty() {
            (keyframe_features[0].width, keyframe_features[0].height)
        } else {
            (0, 0)
        };

        // ─── Temporal Layer 1: Mean Visual ─────────────────────
        // Average of all keyframe patterns (the "typical frame")
        let mut mean_visual = vec![0.0f32; self.pattern_size];
        for pattern in &keyframe_patterns {
            for j in 0..self.pattern_size {
                mean_visual[j] += pattern[j];
            }
        }
        for j in 0..self.pattern_size {
            mean_visual[j] /= n as f32;
        }

        // ─── Temporal Layer 2: Motion Deltas ───────────────────
        // Cosine distance between consecutive keyframes
        let mut motion_deltas = Vec::with_capacity(n.saturating_sub(1));
        for i in 1..n {
            let sim = Self::similarity(&keyframe_patterns[i - 1], &keyframe_patterns[i]);
            // Convert similarity to distance (0 = identical, 2 = opposite)
            motion_deltas.push(1.0 - sim);
        }
        // Pad to 64 bins by interpolation/resampling
        let motion_64 = Self::resample_to_64(&motion_deltas);

        // ─── Temporal Layer 3: Temporal Arc ────────────────────
        // How brightness and contrast evolve over the video.
        // Combine brightness + contrast into a single "visual energy" metric.
        let arc_raw: Vec<f32> = keyframe_features.iter()
            .map(|f| f.mean_brightness * 0.6 + f.contrast * 0.4)
            .collect();
        let temporal_arc = Self::resample_to_64(&arc_raw);

        // ─── Temporal Layer 4: Scene Variance ──────────────────
        // How much each keyframe differs from the mean visual.
        let variance_raw: Vec<f32> = keyframe_patterns.iter()
            .map(|p| {
                let sim = Self::similarity(p, &mean_visual);
                1.0 - sim // distance from mean
            })
            .collect();
        let scene_variance = Self::resample_to_64(&variance_raw);

        let features = VideoFeatures {
            keyframe_features,
            mean_visual: mean_visual.clone(),
            motion_deltas: motion_64.clone(),
            temporal_arc: temporal_arc.clone(),
            scene_variance: scene_variance.clone(),
            num_keyframes: n,
            total_frames: keyframe_bytes.len(),
            width,
            height,
        };

        // Build the final video pattern
        let pattern = self.video_features_to_pattern(&features);

        Some((pattern, features))
    }

    /// Resample a variable-length Vec<f32> to exactly 64 bins.
    /// Uses linear interpolation.
    fn resample_to_64(input: &[f32]) -> Vec<f32> {
        let target = 64;
        if input.is_empty() {
            return vec![0.0; target];
        }
        if input.len() == 1 {
            return vec![input[0]; target];
        }

        let mut output = Vec::with_capacity(target);
        let scale = (input.len() - 1) as f64 / (target - 1) as f64;

        for i in 0..target {
            let src = i as f64 * scale;
            let lo = src.floor() as usize;
            let hi = (lo + 1).min(input.len() - 1);
            let frac = (src - lo as f64) as f32;
            output.push(input[lo] * (1.0 - frac) + input[hi] * frac);
        }

        // Normalize to 0..1
        let max = output.iter().cloned().fold(0.0f32, f32::max);
        if max > 0.0 {
            for v in &mut output {
                *v /= max;
            }
        }

        output
    }

    /// Convert VideoFeatures into a 256-dim pattern vector.
    /// Starts from the mean visual pattern, then adds temporal layers.
    fn video_features_to_pattern(&self, features: &VideoFeatures) -> Vec<f32> {
        let mut pattern = vec![0.0f32; self.pattern_size];

        let vid_seed = self.seed.derive(b"video_encoder");

        // ─── Base: Mean visual (weighted sum of keyframe patterns) ─
        // Start with the mean visual as the foundation
        for j in 0..self.pattern_size {
            pattern[j] += features.mean_visual[j] * 0.5;
        }

        // ─── Temporal Layer 1: Motion delta contribution ───────
        let motion_seed = vid_seed.derive(b"motion_delta");
        for (bin_idx, &delta) in features.motion_deltas.iter().enumerate() {
            if delta < 1e-6 { continue; }
            let bin_seed = motion_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 11) % 32;
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * delta;
            }
        }

        // ─── Temporal Layer 2: Temporal arc contribution ───────
        let arc_seed = vid_seed.derive(b"temporal_arc");
        for (bin_idx, &arc) in features.temporal_arc.iter().enumerate() {
            if arc < 1e-6 { continue; }
            let bin_seed = arc_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 19) % 32;
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * arc;
            }
        }

        // ─── Temporal Layer 3: Scene variance contribution ─────
        let var_seed = vid_seed.derive(b"scene_variance");
        for (bin_idx, &var) in features.scene_variance.iter().enumerate() {
            if var < 1e-6 { continue; }
            let bin_seed = var_seed.derive(&(bin_idx as u16).to_le_bytes());
            let bin_bytes = bin_seed.as_bytes();
            for j in 0..self.pattern_size {
                let idx1 = j % 32;
                let idx2 = (j + 23) % 32;
                let val = ((bin_bytes[idx1] as f32 + bin_bytes[idx2] as f32)
                    / 510.0) * 0.4 - 0.2;
                pattern[j] += val * var;
            }
        }

        // ─── Modality offset ───────────────────────────────────
        let modality_seed = self.seed.derive(b"modality_video");
        let mod_bytes = modality_seed.as_bytes();
        for j in 0..self.pattern_size {
            let idx = j % 32;
            let offset = (mod_bytes[idx] as f32 / 255.0) * 0.04 - 0.02;
            pattern[j] += offset;
        }

        // ─── L2 Normalize to unit vector ───────────────────────
        let norm: f32 = pattern.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut pattern {
                *x /= norm;
            }
        }

        pattern
    }

    /// How similar are two patterns? Cosine similarity.
    /// -1.0 (opposite) to 1.0 (identical).
    /// Works across modalities — an image pattern and a text pattern
    /// can be compared directly since both are unit vectors in
    /// the same 256-dim space.
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() { return 0.0; }
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Learn from association: nudge encoder so associated
    /// inputs produce more similar patterns. This is how
    /// the dialect evolves.
    pub fn learn_from_association(&mut self, input_a: &str, input_b: &str, strength: f32) {
        let pattern_a = self.encode(input_a);
        let pattern_b = self.encode(input_b);

        let bytes_a: Vec<usize> = input_a.bytes().map(|b| b as usize).collect();
        let bytes_b: Vec<usize> = input_b.bytes().map(|b| b as usize).collect();

        for &b in &bytes_a {
            for j in 0..self.pattern_size {
                let direction = pattern_b[j] - self.encoder[b][j];
                self.refinements[b][j] += direction * strength;
            }
        }

        for &b in &bytes_b {
            for j in 0..self.pattern_size {
                let direction = pattern_a[j] - self.encoder[b][j];
                self.refinements[b][j] += direction * strength;
            }
        }
    }
    pub fn pattern_size(&self) -> usize {
        self.pattern_size
    }

    pub fn experiences(&self) -> u64 {
        self.experiences
    }

    pub fn total_bytes_seen(&self) -> u64 {
        self.total_bytes_seen
    }

    pub fn images_encoded(&self) -> u64 {
        self.images_encoded
    }

    pub fn audios_encoded(&self) -> u64 {
        self.audios_encoded
    }

    pub fn videos_encoded(&self) -> u64 {
        self.videos_encoded
    }
}