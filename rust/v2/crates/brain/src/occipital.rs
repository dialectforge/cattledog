// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Occipital lobe — visual processing, image encoding, and media creation.
//!
//! Handles image ingestion: feature extraction → embedding → node creation.
//! Also provides SVG generation and KU-to-diagram conversion.

use std::collections::HashMap;

// ── ImageEncoding ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ImageEncoding {
    pub path: String,
    pub embedding: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    /// Dominant colours as RGB tuples (up to 4 — one per image quadrant).
    pub dominant_colors: Vec<(u8, u8, u8)>,
    pub mean_brightness: f32,
    pub file_size_bytes: u64,
}

// ── OccipitalLobe ─────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct OccipitalLobe {
    /// Path → encoding cache.
    pub cache: HashMap<String, ImageEncoding>,
    pub model_loaded: bool,
    pub images_processed: u64,
}

impl OccipitalLobe {
    pub fn new() -> Self {
        OccipitalLobe::default()
    }

    /// Encode an image at `path` and return its embedding.
    ///
    /// Reads the image file, extracts real spatial/colour/brightness features,
    /// and derives a 384-dim embedding from those features.
    pub fn encode_image(&mut self, path: &str) -> Result<ImageEncoding, String> {
        if let Some(cached) = self.cache.get(path) {
            return Ok(cached.clone());
        }

        let enc = encode_image_from_path(path)?;
        self.cache.insert(path.to_string(), enc.clone());
        self.images_processed += 1;
        Ok(enc)
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    pub fn evict_cache(&mut self) {
        self.cache.clear();
    }
}

// ── Real image encoder ────────────────────────────────────────────────────────

fn encode_image_from_path(path: &str) -> Result<ImageEncoding, String> {
    use image::GenericImageView;

    let file_size_bytes = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    let format_str = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();

    let img = image::open(path).map_err(|e| format!("image open error: {}", e))?;
    let (width, height) = img.dimensions();

    // Convert to RGBA8 for uniform processing.
    let rgba = img.to_rgba8();

    // ── Feature extraction ─────────────────────────────────────────────────
    // Divide image into a 4×4 grid of cells; compute mean R, G, B per cell.
    let cells_x: u32 = 4;
    let cells_y: u32 = 4;
    let cell_w = (width.max(1)) / cells_x.max(1);
    let cell_h = (height.max(1)) / cells_y.max(1);

    let mut cell_rgb: Vec<(f32, f32, f32)> = Vec::with_capacity(16);

    for cy in 0..cells_y {
        for cx in 0..cells_x {
            let x0 = (cx * cell_w).min(width);
            let y0 = (cy * cell_h).min(height);
            let x1 = ((cx + 1) * cell_w).min(width);
            let y1 = ((cy + 1) * cell_h).min(height);

            let mut sum_r = 0f64;
            let mut sum_g = 0f64;
            let mut sum_b = 0f64;
            let mut count = 0u64;

            for y in y0..y1 {
                for x in x0..x1 {
                    let p = rgba.get_pixel(x, y);
                    sum_r += p[0] as f64;
                    sum_g += p[1] as f64;
                    sum_b += p[2] as f64;
                    count += 1;
                }
            }

            if count > 0 {
                cell_rgb.push((
                    (sum_r / count as f64) as f32,
                    (sum_g / count as f64) as f32,
                    (sum_b / count as f64) as f32,
                ));
            } else {
                cell_rgb.push((0.0, 0.0, 0.0));
            }
        }
    }

    // ── Dominant colours (quadrant means) ─────────────────────────────────
    // Take the 4 corner cells as the "dominant" colours.
    let quadrant_indices = [0usize, 3, 12, 15];
    let dominant_colors: Vec<(u8, u8, u8)> = quadrant_indices
        .iter()
        .filter_map(|&i| cell_rgb.get(i))
        .map(|(r, g, b)| (*r as u8, *g as u8, *b as u8))
        .collect();

    // ── Mean brightness ────────────────────────────────────────────────────
    let total_brightness: f32 = cell_rgb.iter().map(|(r, g, b)| (r + g + b) / 3.0).sum();
    let mean_brightness = total_brightness / cell_rgb.len().max(1) as f32 / 255.0;

    // ── Aspect ratio ───────────────────────────────────────────────────────
    let aspect = width as f32 / height.max(1) as f32;

    // ── 384-dim embedding ──────────────────────────────────────────────────
    // Layout:
    //   [0..48]   — 16 cell RGB values normalised to [0,1], packed as (r,g,b) × 16
    //   [48]      — aspect ratio normalised to [0,1] via sigmoid-ish mapping
    //   [49]      — mean brightness [0,1]
    //   [50..51]  — normalised width / height (relative to 4096 max)
    //   [52..383] — padded to 384 dims with decay from cell features
    let mut embedding = vec![0.0f32; 384];

    for (i, (r, g, b)) in cell_rgb.iter().enumerate() {
        let base = i * 3;
        if base + 2 < 384 {
            embedding[base]     = r / 255.0;
            embedding[base + 1] = g / 255.0;
            embedding[base + 2] = b / 255.0;
        }
    }
    // 48..51
    embedding[48] = (aspect / (1.0 + aspect)).clamp(0.0, 1.0);
    embedding[49] = mean_brightness;
    embedding[50] = (width as f32 / 4096.0).clamp(0.0, 1.0);
    embedding[51] = (height as f32 / 4096.0).clamp(0.0, 1.0);

    // Fill remaining dims [52..384] with harmonic echoes of the cell features.
    // This spreads information across the embedding space without being
    // purely random, giving cosine similarity meaning for similar images.
    for i in 52..384 {
        let src_idx = (i - 52) % 48;
        let harmonic = (i as f32 * std::f32::consts::PI / 52.0).sin();
        embedding[i] = embedding[src_idx] * (0.5 + 0.5 * harmonic);
    }

    Ok(ImageEncoding {
        path: path.to_string(),
        embedding,
        width,
        height,
        format: format_str,
        dominant_colors,
        mean_brightness,
        file_size_bytes,
    })
}

// ── SVG generation ────────────────────────────────────────────────────────────

/// A simple SVG element.
#[derive(Debug, Clone)]
pub enum SvgElement {
    Rect { x: f32, y: f32, w: f32, h: f32, fill: String, stroke: String },
    Circle { cx: f32, cy: f32, r: f32, fill: String },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, stroke: String, stroke_width: f32 },
    Text { x: f32, y: f32, text: String, font_size: f32, fill: String },
    Arrow { x1: f32, y1: f32, x2: f32, y2: f32, stroke: String },
}

impl SvgElement {
    fn to_svg(&self) -> String {
        match self {
            SvgElement::Rect { x, y, w, h, fill, stroke } => format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5" rx="4"/>"#,
                x, y, w, h, fill, stroke
            ),
            SvgElement::Circle { cx, cy, r, fill } => format!(
                r#"  <circle cx="{}" cy="{}" r="{}" fill="{}"/>"#,
                cx, cy, r, fill
            ),
            SvgElement::Line { x1, y1, x2, y2, stroke, stroke_width } => format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                x1, y1, x2, y2, stroke, stroke_width
            ),
            SvgElement::Text { x, y, text, font_size, fill } => format!(
                r#"  <text x="{}" y="{}" font-size="{}" fill="{}" font-family="sans-serif" text-anchor="middle">{}</text>"#,
                x, y, font_size, fill, escape_xml(text)
            ),
            SvgElement::Arrow { x1, y1, x2, y2, stroke } => {
                // Arrow = line + small arrowhead triangle at (x2, y2).
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let ux = dx / len;
                let uy = dy / len;
                let hs = 10.0f32; // half-size of arrowhead
                let px = x2 - ux * hs - uy * hs * 0.5;
                let py = y2 - uy * hs + ux * hs * 0.5;
                let qx = x2 - ux * hs + uy * hs * 0.5;
                let qy = y2 - uy * hs - ux * hs * 0.5;
                format!(
                    r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>
  <polygon points="{},{} {},{} {},{}" fill="{}"/>"#,
                    x1, y1, x2, y2, stroke,
                    x2, y2, px, py, qx, qy, stroke
                )
            }
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// A builder for SVG diagrams.
#[derive(Debug, Default)]
pub struct SvgBuilder {
    pub width: f32,
    pub height: f32,
    pub background: String,
    elements: Vec<SvgElement>,
}

impl SvgBuilder {
    pub fn new(width: f32, height: f32) -> Self {
        SvgBuilder {
            width,
            height,
            background: "#ffffff".to_string(),
            elements: Vec::new(),
        }
    }

    pub fn add(&mut self, el: SvgElement) -> &mut Self {
        self.elements.push(el);
        self
    }

    pub fn build(&self) -> String {
        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            self.width, self.height, self.width, self.height
        );
        svg.push('\n');
        svg.push_str(&format!(
            r#"  <rect width="{}" height="{}" fill="{}"/>"#,
            self.width, self.height, self.background
        ));
        svg.push('\n');
        for el in &self.elements {
            svg.push_str(&el.to_svg());
            svg.push('\n');
        }
        svg.push_str("</svg>");
        svg
    }
}

/// Generate a simple concept-map SVG for a list of (label, description) nodes
/// and (from_idx, to_idx, label) edges.
///
/// Nodes are arranged in a circle; edges are drawn as arrows.
pub fn generate_concept_map(
    nodes: &[(String, String)],
    edges: &[(usize, usize, String)],
) -> String {
    if nodes.is_empty() {
        return generate_placeholder_svg("empty concept map");
    }

    let w = 600.0f32;
    let h = 500.0f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let radius = 180.0f32;

    let mut svg = SvgBuilder::new(w, h);
    svg.background = "#f8f9fa".to_string();

    let n = nodes.len();

    // Pre-compute node positions.
    let positions: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let angle = (i as f32 / n as f32) * 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
            (cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect();

    // Draw edges first (so they appear behind nodes).
    for (from, to, label) in edges {
        if let (Some(&(x1, y1)), Some(&(x2, y2))) =
            (positions.get(*from), positions.get(*to))
        {
            svg.add(SvgElement::Arrow {
                x1, y1, x2, y2,
                stroke: "#999999".to_string(),
            });
            // Edge label at midpoint.
            if !label.is_empty() {
                svg.add(SvgElement::Text {
                    x: (x1 + x2) / 2.0,
                    y: (y1 + y2) / 2.0 - 6.0,
                    text: label.clone(),
                    font_size: 10.0,
                    fill: "#666666".to_string(),
                });
            }
        }
    }

    // Draw nodes.
    let colors = ["#4a90e2", "#7ed321", "#d0021b", "#f5a623", "#9013fe", "#50e3c2"];
    for (i, (label, _desc)) in nodes.iter().enumerate() {
        let (nx, ny) = positions[i];
        let color = colors[i % colors.len()];
        let text_len = label.len() as f32;
        let box_w = (text_len * 7.5 + 20.0).max(80.0).min(140.0);
        let box_h = 34.0f32;

        svg.add(SvgElement::Rect {
            x: nx - box_w / 2.0,
            y: ny - box_h / 2.0,
            w: box_w,
            h: box_h,
            fill: color.to_string(),
            stroke: "#333333".to_string(),
        });
        svg.add(SvgElement::Text {
            x: nx,
            y: ny + 5.0,
            text: label.clone(),
            font_size: 12.0,
            fill: "#ffffff".to_string(),
        });
    }

    svg.build()
}

/// Generate a simple flowchart SVG from a list of steps.
pub fn generate_flowchart(title: &str, steps: &[String]) -> String {
    if steps.is_empty() {
        return generate_placeholder_svg("empty flowchart");
    }

    let w = 400.0f32;
    let step_h = 50.0f32;
    let gap = 30.0f32;
    let h = (steps.len() as f32) * (step_h + gap) + 80.0;

    let mut svg = SvgBuilder::new(w, h);
    svg.background = "#ffffff".to_string();

    // Title
    svg.add(SvgElement::Text {
        x: w / 2.0,
        y: 28.0,
        text: title.to_string(),
        font_size: 15.0,
        fill: "#222222".to_string(),
    });

    let box_w = 260.0f32;
    let box_x = (w - box_w) / 2.0;

    for (i, step) in steps.iter().enumerate() {
        let y = 50.0 + i as f32 * (step_h + gap);

        svg.add(SvgElement::Rect {
            x: box_x,
            y,
            w: box_w,
            h: step_h,
            fill: "#e8f4fd".to_string(),
            stroke: "#4a90e2".to_string(),
        });
        svg.add(SvgElement::Text {
            x: w / 2.0,
            y: y + step_h / 2.0 + 5.0,
            text: format!("{}. {}", i + 1, step),
            font_size: 11.0,
            fill: "#333333".to_string(),
        });

        // Arrow to next step
        if i + 1 < steps.len() {
            let arrow_y1 = y + step_h;
            let arrow_y2 = y + step_h + gap;
            svg.add(SvgElement::Arrow {
                x1: w / 2.0,
                y1: arrow_y1,
                x2: w / 2.0,
                y2: arrow_y2,
                stroke: "#4a90e2".to_string(),
            });
        }
    }

    svg.build()
}

/// Generate a minimal placeholder SVG with a label.
pub fn generate_placeholder_svg(label: &str) -> String {
    let mut svg = SvgBuilder::new(200.0, 80.0);
    svg.background = "#f0f0f0".to_string();
    svg.add(SvgElement::Text {
        x: 100.0,
        y: 45.0,
        text: label.to_string(),
        font_size: 13.0,
        fill: "#888888".to_string(),
    });
    svg.build()
}

/// Generate a Mermaid-compatible diagram syntax from a topic and KU descriptions.
///
/// Returns a Mermaid `graph TD` (top-down) snippet.
pub fn to_mermaid_diagram(topic: &str, ku_descriptions: &[(String, String)]) -> String {
    let mut lines = vec!["graph TD".to_string()];
    let topic_id = sanitize_mermaid_id(topic);
    lines.push(format!("  {}[\"{}\"]", topic_id, escape_mermaid(topic)));

    for (i, (ku_type, text)) in ku_descriptions.iter().enumerate() {
        let node_id = format!("ku{}", i);
        let short = if text.len() > 50 {
            format!("{}…", &text[..50])
        } else {
            text.clone()
        };
        lines.push(format!(r#"  {}["{}"]"#, node_id, escape_mermaid(&short)));
        lines.push(format!("  {} -->|{}| {}", topic_id, escape_mermaid(ku_type), node_id));
    }

    lines.join("\n")
}

fn sanitize_mermaid_id(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(20)
        .collect::<String>()
        .to_lowercase()
}

fn escape_mermaid(s: &str) -> String {
    s.replace('"', "'")
}
