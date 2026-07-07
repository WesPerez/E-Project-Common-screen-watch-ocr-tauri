use crate::config::{parse_scales, ConfigError, TargetConfig, WatchConfig};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BinaryHeap, HashMap},
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
    sync::OnceLock,
    thread,
};
use thiserror::Error;

const COARSE_AREA: u64 = 1280 * 720;
const QUARTER_AREA: u64 = 3840 * 2160;
const COARSE_CANDIDATES: usize = 3;
const TEXTURED_COARSE_CANDIDATES: usize = 8;
const COARSE_CANDIDATE_POOL_MULTIPLIER: usize = 8;
const REFINE_MARGIN: u32 = 16;
const EXACT_GRAY_MAX_POSITIONS: u64 = 250_000;
const PERFECT_SCORE_THRESHOLD: f32 = 1.0 - 1e-6;

#[derive(Debug, Error)]
pub enum DetectError {
    #[error("invalid detector config: {0}")]
    Config(#[from] ConfigError),
    #[error("cannot read template {path}: {source}")]
    TemplateIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot decode template {path}: {message}")]
    TemplateDecode { path: PathBuf, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub target: String,
    pub target_id: String,
    pub kind: String,
    pub score: f32,
    pub box_xyxy: [u32; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrTextRow {
    pub text: String,
    pub score: f32,
    pub box_points: Option<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateMatch {
    pub score: f32,
    pub box_xyxy: [u32; 4],
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct GrayFrame {
    width: u32,
    height: u32,
    pixels: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
struct CoarseTemplate {
    phase_x: u32,
    phase_y: u32,
    frame: GrayFrame,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CandidateLoc {
    score: f32,
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ExactGrayAnchor {
    x: u32,
    y: u32,
    value: f32,
}

impl Eq for CandidateLoc {}

impl Ord for CandidateLoc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| other.y.cmp(&self.y))
            .then_with(|| other.x.cmp(&self.x))
    }
}

impl PartialOrd for CandidateLoc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SparseTemplateSamples {
    samples: Vec<(u32, u32, f32)>,
    mean: f32,
    energy: f32,
}

impl SparseTemplateSamples {
    fn from_template(template: &GrayFrame) -> Option<Self> {
        let xs = sample_positions(template.width);
        let ys = sample_positions(template.height);
        let mut samples = Vec::with_capacity(xs.len() * ys.len());
        for y in ys {
            for x in &xs {
                samples.push((*x, y, template.pixel(*x, y)));
            }
        }
        if samples.is_empty() {
            return None;
        }
        let mean = samples.iter().map(|(_, _, value)| *value).sum::<f32>() / samples.len() as f32;
        let energy = samples
            .iter()
            .map(|(_, _, value)| {
                let delta = *value - mean;
                delta * delta
            })
            .sum();
        if energy <= f32::EPSILON {
            None
        } else {
            Some(Self {
                samples,
                mean,
                energy,
            })
        }
    }

    fn score_at(&self, frame: &GrayFrame, left: u32, top: u32) -> f32 {
        let window_mean = self
            .samples
            .iter()
            .map(|(x, y, _)| frame.pixel(left + *x, top + *y))
            .sum::<f32>()
            / self.samples.len() as f32;
        let mut numerator = 0.0;
        let mut window_energy = 0.0;
        for (x, y, template_value) in &self.samples {
            let window_delta = frame.pixel(left + *x, top + *y) - window_mean;
            let template_delta = *template_value - self.mean;
            numerator += window_delta * template_delta;
            window_energy += window_delta * window_delta;
        }
        if window_energy <= f32::EPSILON {
            return -1.0;
        }
        (numerator / (window_energy.sqrt() * self.energy.sqrt())).clamp(-1.0, 1.0)
    }
}

fn sample_positions(size: u32) -> Vec<u32> {
    if size == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(3);
    for value in [0, size / 2, size - 1] {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedDetector {
    targets: Vec<PreparedTarget>,
    template_workers: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum PreparedTarget {
    Pixel {
        name: String,
        id: Option<String>,
        x: u32,
        y: u32,
        rgb: [u8; 3],
        tolerance: u8,
    },
    Template {
        name: String,
        id: Option<String>,
        threshold: f32,
        scales: Vec<f64>,
        template: GrayFrame,
    },
    OcrText {
        name: String,
        id: Option<String>,
        text: String,
        min_score: f32,
        case_sensitive: bool,
    },
}

impl RgbFrame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let expected = width as usize * height as usize * 3;
        if pixels.len() != expected {
            return Err(format!(
                "expected {expected} RGB bytes, got {}",
                pixels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn from_png_path(path: impl AsRef<Path>) -> Result<Self, DetectError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| DetectError::TemplateIo {
            path: path.to_path_buf(),
            source,
        })?;
        let decoder = png::Decoder::new(BufReader::new(file));
        let mut reader = decoder
            .read_info()
            .map_err(|err| template_decode_error(path, err))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|err| template_decode_error(path, err))?;
        if info.bit_depth != png::BitDepth::Eight {
            return Err(DetectError::TemplateDecode {
                path: path.to_path_buf(),
                message: format!("unsupported PNG bit depth {:?}", info.bit_depth),
            });
        }
        let bytes = &buf[..info.buffer_size()];
        let pixels = match info.color_type {
            png::ColorType::Rgb => bytes.to_vec(),
            png::ColorType::Rgba => bytes
                .chunks_exact(4)
                .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
                .collect(),
            png::ColorType::Grayscale => bytes
                .iter()
                .flat_map(|value| [*value, *value, *value])
                .collect(),
            png::ColorType::GrayscaleAlpha => bytes
                .chunks_exact(2)
                .flat_map(|ga| [ga[0], ga[0], ga[0]])
                .collect(),
            png::ColorType::Indexed => {
                return Err(DetectError::TemplateDecode {
                    path: path.to_path_buf(),
                    message: "indexed PNG templates are not supported yet".to_string(),
                })
            }
        };
        Self::new(info.width, info.height, pixels).map_err(|message| DetectError::TemplateDecode {
            path: path.to_path_buf(),
            message,
        })
    }

    pub fn from_image_path(path: impl AsRef<Path>) -> Result<Self, DetectError> {
        let path = path.as_ref();
        let reader = image::ImageReader::open(path).map_err(|source| DetectError::TemplateIo {
            path: path.to_path_buf(),
            source,
        })?;
        let image = reader
            .with_guessed_format()
            .map_err(|err| DetectError::TemplateDecode {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?
            .decode()
            .map_err(|err| DetectError::TemplateDecode {
                path: path.to_path_buf(),
                message: err.to_string(),
            })?
            .to_rgb8();
        Self::new(image.width(), image.height(), image.into_raw()).map_err(|message| {
            DetectError::TemplateDecode {
                path: path.to_path_buf(),
                message,
            }
        })
    }

    pub fn from_image_bytes(label: impl Into<PathBuf>, bytes: &[u8]) -> Result<Self, DetectError> {
        let path = label.into();
        let image = image::load_from_memory(bytes)
            .map_err(|err| DetectError::TemplateDecode {
                path: path.clone(),
                message: err.to_string(),
            })?
            .to_rgb8();
        Self::new(image.width(), image.height(), image.into_raw())
            .map_err(|message| DetectError::TemplateDecode { path, message })
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = ((y * self.width + x) * 3) as usize;
        Some([
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
        ])
    }
}

impl PreparedDetector {
    pub fn from_config(
        config: &WatchConfig,
        base_dir: impl AsRef<Path>,
    ) -> Result<Self, DetectError> {
        config.validate()?;
        let base_dir = base_dir.as_ref();
        let mut targets = Vec::with_capacity(config.targets.len());
        for target in &config.targets {
            let prepared = match target {
                TargetConfig::Pixel {
                    name,
                    x,
                    y,
                    rgb,
                    tolerance,
                    id,
                    ..
                } => PreparedTarget::Pixel {
                    name: name.clone(),
                    id: id.clone(),
                    x: *x,
                    y: *y,
                    rgb: *rgb,
                    tolerance: *tolerance,
                },
                TargetConfig::Template {
                    name,
                    path,
                    threshold,
                    scales,
                    id,
                    ..
                } => {
                    let path = resolve_path(base_dir, path);
                    let template = RgbFrame::from_image_path(&path)?;
                    PreparedTarget::Template {
                        name: name.clone(),
                        id: id.clone(),
                        threshold: *threshold,
                        scales: parse_scales(scales)?,
                        template: GrayFrame::from_rgb(&template),
                    }
                }
                TargetConfig::OcrText {
                    name,
                    text,
                    min_score,
                    case_sensitive,
                    id,
                    ..
                } => PreparedTarget::OcrText {
                    name: name.clone(),
                    id: id.clone(),
                    text: text.clone(),
                    min_score: *min_score,
                    case_sensitive: *case_sensitive,
                },
                TargetConfig::Unknown => PreparedTarget::OcrText {
                    name: String::new(),
                    id: None,
                    text: String::new(),
                    min_score: 0.0,
                    case_sensitive: false,
                },
            };
            targets.push(prepared);
        }
        Ok(Self {
            targets,
            template_workers: config.template_worker_limit(),
        })
    }

    pub fn run(&self, frame: &RgbFrame) -> Vec<Match> {
        self.run_with_optional_ocr_rows(frame, None)
    }

    pub fn run_with_ocr_rows(&self, frame: &RgbFrame, ocr_rows: &[OcrTextRow]) -> Vec<Match> {
        self.run_with_optional_ocr_rows(frame, Some(ocr_rows))
    }

    fn run_with_optional_ocr_rows(
        &self,
        frame: &RgbFrame,
        ocr_rows: Option<&[OcrTextRow]>,
    ) -> Vec<Match> {
        let mut hits = vec![None; self.targets.len()];
        let mut template_jobs = Vec::new();
        for (index, target) in self.targets.iter().enumerate() {
            match target {
                PreparedTarget::Pixel {
                    name,
                    id,
                    x,
                    y,
                    rgb,
                    tolerance,
                } => {
                    hits[index] =
                        detect_pixel(frame, name, id.as_deref(), *x, *y, *rgb, *tolerance);
                }
                PreparedTarget::Template { .. } => {
                    template_jobs.push((index, target));
                }
                PreparedTarget::OcrText {
                    name,
                    id,
                    text,
                    min_score,
                    case_sensitive,
                } => {
                    hits[index] = ocr_rows.and_then(|rows| {
                        detect_ocr_text(
                            rows,
                            name,
                            id.as_deref(),
                            text,
                            *min_score,
                            *case_sensitive,
                        )
                    });
                }
            }
        }

        for (index, hit) in run_template_jobs(frame, &template_jobs, self.template_workers) {
            hits[index] = hit;
        }
        hits.into_iter().flatten().collect()
    }

    pub fn template_worker_limit(&self) -> usize {
        self.template_workers
    }
}

pub fn detect_targets(frame: &RgbFrame, targets: &[TargetConfig]) -> Vec<Match> {
    targets
        .iter()
        .filter_map(|target| match target {
            TargetConfig::Pixel {
                name,
                x,
                y,
                rgb,
                tolerance,
                id,
                ..
            } => detect_pixel(frame, name, id.as_deref(), *x, *y, *rgb, *tolerance),
            TargetConfig::Template { .. }
            | TargetConfig::OcrText { .. }
            | TargetConfig::Unknown => None,
        })
        .collect()
}

pub fn detect_ocr_text_targets(rows: &[OcrTextRow], targets: &[TargetConfig]) -> Vec<Match> {
    targets
        .iter()
        .filter_map(|target| match target {
            TargetConfig::OcrText {
                name,
                text,
                min_score,
                case_sensitive,
                id,
                ..
            } => detect_ocr_text(rows, name, id.as_deref(), text, *min_score, *case_sensitive),
            TargetConfig::Pixel { .. } | TargetConfig::Template { .. } | TargetConfig::Unknown => {
                None
            }
        })
        .collect()
}

pub fn detect_pixel(
    frame: &RgbFrame,
    name: &str,
    id: Option<&str>,
    x: u32,
    y: u32,
    expected: [u8; 3],
    tolerance: u8,
) -> Option<Match> {
    let actual = frame.pixel(x, y)?;
    let dist = actual
        .into_iter()
        .zip(expected)
        .map(|(actual, expected)| actual.abs_diff(expected))
        .max()
        .unwrap_or(0);
    if dist <= tolerance {
        Some(Match {
            target: name.to_string(),
            target_id: id.unwrap_or(name).to_string(),
            kind: "pixel".to_string(),
            score: 1.0 - f32::from(dist) / 255.0,
            box_xyxy: [
                x.saturating_sub(4),
                y.saturating_sub(4),
                x.saturating_add(4),
                y.saturating_add(4),
            ],
            scale: None,
            text: None,
        })
    } else {
        None
    }
}

pub fn detect_ocr_text(
    rows: &[OcrTextRow],
    name: &str,
    id: Option<&str>,
    needle: &str,
    min_score: f32,
    case_sensitive: bool,
) -> Option<Match> {
    let wanted = if case_sensitive {
        needle.to_string()
    } else {
        needle.to_lowercase()
    };
    for row in rows {
        let haystack = if case_sensitive {
            row.text.clone()
        } else {
            row.text.to_lowercase()
        };
        if row.score >= min_score && haystack.contains(&wanted) {
            return Some(Match {
                target: name.to_string(),
                target_id: id.unwrap_or(name).to_string(),
                kind: "ocr_text".to_string(),
                score: row.score,
                box_xyxy: row
                    .box_points
                    .as_deref()
                    .and_then(flatten_ocr_box)
                    .unwrap_or([0, 0, 0, 0]),
                scale: None,
                text: Some(row.text.clone()),
            });
        }
    }
    None
}

pub fn detect_template_scaled(
    frame: &RgbFrame,
    name: &str,
    id: Option<&str>,
    template: &RgbFrame,
    threshold: f32,
    scales: &[f64],
) -> Option<Match> {
    let hit = find_template_scaled(frame, template, scales)?;
    if hit.score < threshold {
        return None;
    }
    Some(Match {
        target: name.to_string(),
        target_id: id.unwrap_or(name).to_string(),
        kind: "template".to_string(),
        score: hit.score,
        box_xyxy: hit.box_xyxy,
        scale: Some(hit.scale),
        text: None,
    })
}

pub fn find_template_scaled(
    frame: &RgbFrame,
    template: &RgbFrame,
    scales: &[f64],
) -> Option<TemplateMatch> {
    if scales.is_empty() {
        return None;
    }
    let base_template = GrayFrame::from_rgb(template);
    let frame_cache = GrayFrameCache::from_rgb(frame);
    find_template_scaled_with_cache(&frame_cache, &base_template, scales)
}

fn find_template_scaled_with_cache(
    frame_cache: &GrayFrameCache,
    base_template: &GrayFrame,
    scales: &[f64],
) -> Option<TemplateMatch> {
    let mut best: Option<TemplateMatch> = None;
    for scale in scales {
        if *scale <= 0.0 {
            continue;
        }
        let scaled_template = base_template.resize_by(*scale)?;
        let hit = find_template_best_gray_adaptive(frame_cache, &scaled_template).map(|mut hit| {
            hit.scale = (*scale * 1_000_000.0).round() / 1_000_000.0;
            hit
        });
        if let Some(hit) = hit {
            if best
                .as_ref()
                .map(|current| hit.score > current.score)
                .unwrap_or(true)
            {
                best = Some(hit);
            }
        }
    }
    best
}

pub fn find_template_exact(frame: &RgbFrame, template: &RgbFrame) -> Option<[u32; 4]> {
    if template.width == 0
        || template.height == 0
        || template.width > frame.width
        || template.height > frame.height
    {
        return None;
    }

    for y in 0..=(frame.height - template.height) {
        for x in 0..=(frame.width - template.width) {
            if template_matches_at(frame, template, x, y) {
                return Some([x, y, x + template.width, y + template.height]);
            }
        }
    }
    None
}

fn template_matches_at(frame: &RgbFrame, template: &RgbFrame, left: u32, top: u32) -> bool {
    for ty in 0..template.height {
        for tx in 0..template.width {
            if frame.pixel(left + tx, top + ty) != template.pixel(tx, ty) {
                return false;
            }
        }
    }
    true
}

fn flatten_ocr_box(points: &[[f32; 2]]) -> Option<[u32; 4]> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut seen = false;
    for [x, y] in points {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        seen = true;
        min_x = min_x.min(*x);
        min_y = min_y.min(*y);
        max_x = max_x.max(*x);
        max_y = max_y.max(*y);
    }
    if !seen {
        return None;
    }
    Some([
        min_x.max(0.0) as u32,
        min_y.max(0.0) as u32,
        max_x.max(0.0) as u32,
        max_y.max(0.0) as u32,
    ])
}

fn run_template_jobs(
    frame: &RgbFrame,
    jobs: &[(usize, &PreparedTarget)],
    template_workers: usize,
) -> Vec<(usize, Option<Match>)> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let frame_cache = GrayFrameCache::from_rgb(frame);
    let worker_count = template_worker_count(template_workers, jobs.len());
    if worker_count <= 1 {
        return jobs
            .iter()
            .map(|(index, target)| (*index, detect_prepared_template(&frame_cache, target)))
            .collect();
    }

    let chunk_size = jobs.len().div_ceil(worker_count);
    let frame_cache = &frame_cache;
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in jobs.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|(index, target)| (*index, detect_prepared_template(&frame_cache, target)))
                    .collect::<Vec<_>>()
            }));
        }

        let mut out = Vec::with_capacity(jobs.len());
        for handle in handles {
            out.extend(handle.join().expect("template worker thread panicked"));
        }
        out
    })
}

fn template_worker_count(template_workers: usize, template_job_count: usize) -> usize {
    if template_job_count == 0 {
        0
    } else if template_job_count == 1 {
        1
    } else {
        template_workers.max(1).min(template_job_count)
    }
}

fn detect_prepared_template(
    frame_cache: &GrayFrameCache,
    target: &PreparedTarget,
) -> Option<Match> {
    let PreparedTarget::Template {
        name,
        id,
        threshold,
        scales,
        template,
    } = target
    else {
        return None;
    };
    let hit = find_template_scaled_with_cache(frame_cache, template, scales)?;
    if hit.score < *threshold {
        return None;
    }
    Some(Match {
        target: name.to_string(),
        target_id: id.as_deref().unwrap_or(name).to_string(),
        kind: "template".to_string(),
        score: hit.score,
        box_xyxy: hit.box_xyxy,
        scale: Some(hit.scale),
        text: None,
    })
}

fn find_template_best_gray_adaptive(
    frame_cache: &GrayFrameCache,
    template: &GrayFrame,
) -> Option<TemplateMatch> {
    let frame = frame_cache.full();
    if should_try_exact_gray(frame, template) {
        if let Some(hit) =
            find_template_exact_gray_bounded(frame, template, EXACT_GRAY_MAX_POSITIONS)
        {
            return Some(hit);
        }
    }
    let full_integral = frame_cache.integral_for_factor(1.0);
    let Some((factor, coarse_templates)) = coarse_plan(frame, template) else {
        return find_template_best_gray_in_region_cached(
            frame,
            template,
            0,
            0,
            frame.width,
            frame.height,
            full_integral,
        );
    };
    let Some(coarse_frame) = frame_cache.frame_for_factor(factor) else {
        return find_template_best_gray_in_region_cached(
            frame,
            template,
            0,
            0,
            frame.width,
            frame.height,
            full_integral,
        );
    };
    if coarse_templates.is_empty() {
        return find_template_best_gray_in_region_cached(
            frame,
            template,
            0,
            0,
            frame.width,
            frame.height,
            full_integral,
        );
    }

    let mut best: Option<TemplateMatch> = None;
    let coarse_integral = frame_cache.integral_for_factor(factor);
    for coarse_template in &coarse_templates {
        if coarse_template.frame.width == 0
            || coarse_template.frame.height == 0
            || coarse_template.frame.width > coarse_frame.width
            || coarse_template.frame.height > coarse_frame.height
        {
            continue;
        }
        let candidate_limit = if coarse_template.frame.is_flat() {
            COARSE_CANDIDATES
        } else {
            TEXTURED_COARSE_CANDIDATES
        };
        for loc in candidate_locs(
            coarse_frame,
            &coarse_template.frame,
            candidate_limit,
            coarse_integral,
        ) {
            if let Some(hit) = refine_template_match(
                frame,
                template,
                loc,
                factor,
                coarse_template.phase_x,
                coarse_template.phase_y,
                full_integral,
            ) {
                if best
                    .as_ref()
                    .map(|current| hit.score > current.score)
                    .unwrap_or(true)
                {
                    best = Some(hit);
                }
            }
        }
    }
    best
}

fn should_try_exact_gray(frame: &GrayFrame, template: &GrayFrame) -> bool {
    if template.width == 0
        || template.height == 0
        || template.width > frame.width
        || template.height > frame.height
        || template.is_flat()
    {
        return false;
    }
    true
}

#[cfg(test)]
fn find_template_exact_gray(frame: &GrayFrame, template: &GrayFrame) -> Option<TemplateMatch> {
    find_template_exact_gray_bounded(frame, template, u64::MAX)
}

fn find_template_exact_gray_bounded(
    frame: &GrayFrame,
    template: &GrayFrame,
    max_positions: u64,
) -> Option<TemplateMatch> {
    if template.width == 0
        || template.height == 0
        || template.width > frame.width
        || template.height > frame.height
        || max_positions == 0
    {
        return None;
    }

    let anchor = exact_match_anchor(template)?;
    let samples = exact_match_samples(template);
    let max_x = frame.width - template.width;
    let max_y = frame.height - template.height;
    let scan_width = max_x as usize + 1;
    let mut scanned_positions = 0u64;
    for y in 0..=max_y {
        let row_start = ((y + anchor.y) * frame.width + anchor.x) as usize;
        let row = &frame.pixels[row_start..row_start + scan_width];
        'candidate: for (x_offset, value) in row.iter().enumerate() {
            if scanned_positions >= max_positions {
                return None;
            }
            scanned_positions += 1;
            if *value != anchor.value {
                continue;
            }
            let x = x_offset as u32;
            for (sample_x, sample_y, value) in &samples {
                if frame.pixel(x + *sample_x, y + *sample_y) != *value {
                    continue 'candidate;
                }
            }
            if gray_template_matches_at(frame, template, x, y) {
                return Some(TemplateMatch {
                    score: 1.0,
                    box_xyxy: [x, y, x + template.width, y + template.height],
                    scale: 1.0,
                });
            }
        }
    }
    None
}

fn exact_match_anchor(template: &GrayFrame) -> Option<ExactGrayAnchor> {
    if template.width == 0 || template.height == 0 || template.pixels.is_empty() {
        return None;
    }

    let mut counts = HashMap::new();
    for value in &template.pixels {
        *counts.entry(value.to_bits()).or_insert(0usize) += 1;
    }

    let center_x_twice = i64::from(template.width.saturating_sub(1));
    let center_y_twice = i64::from(template.height.saturating_sub(1));
    let mut best: Option<((usize, i64, u32, u32), ExactGrayAnchor)> = None;
    for (index, value) in template.pixels.iter().enumerate() {
        let x = index as u32 % template.width;
        let y = index as u32 / template.width;
        let distance_from_center =
            (i64::from(x) * 2 - center_x_twice).abs() + (i64::from(y) * 2 - center_y_twice).abs();
        let key = (
            *counts.get(&value.to_bits()).unwrap_or(&usize::MAX),
            distance_from_center,
            y,
            x,
        );
        let anchor = ExactGrayAnchor {
            x,
            y,
            value: *value,
        };
        if best
            .as_ref()
            .map(|(best_key, _)| key < *best_key)
            .unwrap_or(true)
        {
            best = Some((key, anchor));
        }
    }
    best.map(|(_, anchor)| anchor)
}

fn exact_match_samples(template: &GrayFrame) -> Vec<(u32, u32, f32)> {
    let mut samples = Vec::with_capacity(9);
    for y in sample_positions(template.height) {
        for x in sample_positions(template.width) {
            samples.push((x, y, template.pixel(x, y)));
        }
    }
    samples
}

fn gray_template_matches_at(frame: &GrayFrame, template: &GrayFrame, left: u32, top: u32) -> bool {
    for ty in 0..template.height {
        let frame_start = ((top + ty) * frame.width + left) as usize;
        let template_start = (ty * template.width) as usize;
        let width = template.width as usize;
        if frame.pixels[frame_start..frame_start + width]
            != template.pixels[template_start..template_start + width]
        {
            return false;
        }
    }
    true
}

fn coarse_plan(frame: &GrayFrame, template: &GrayFrame) -> Option<(f64, Vec<CoarseTemplate>)> {
    let area = u64::from(frame.width) * u64::from(frame.height);
    if area >= QUARTER_AREA {
        if let Some(templates) = coarse_templates_for(template, 0.25) {
            return Some((0.25, templates));
        }
    }
    if area >= COARSE_AREA {
        if let Some(templates) = coarse_templates_for(template, 0.5) {
            return Some((0.5, templates));
        }
    }
    None
}

fn coarse_templates_for(template: &GrayFrame, factor: f64) -> Option<Vec<CoarseTemplate>> {
    let width = ((f64::from(template.width) * factor) as u32).max(1);
    let height = ((f64::from(template.height) * factor) as u32).max(1);
    let min_dim = if (factor - 0.5).abs() < f64::EPSILON {
        3
    } else if (factor - 0.25).abs() < f64::EPSILON {
        4
    } else if (factor - 0.125).abs() < f64::EPSILON {
        8
    } else {
        1
    };
    if width.min(height) < min_dim {
        return None;
    }

    let coarse = template.resize_area_by(factor)?;
    if template.is_flat() || !coarse.is_flat() {
        Some(vec![CoarseTemplate {
            phase_x: 0,
            phase_y: 0,
            frame: coarse,
        }])
    } else {
        None
    }
}

fn candidate_locs(
    frame: &GrayFrame,
    template: &GrayFrame,
    limit: usize,
    cached_integral: Option<&GrayFrameIntegral>,
) -> Vec<(u32, u32)> {
    if limit == 0
        || template.width == 0
        || template.height == 0
        || template.width > frame.width
        || template.height > frame.height
    {
        return Vec::new();
    }

    let radius = (template.width.min(template.height) / 2).max(2);
    let template_flat = template.is_flat();
    let sparse_samples = if template_flat {
        None
    } else {
        SparseTemplateSamples::from_template(template)
    };
    let template_stats = sparse_samples
        .is_none()
        .then(|| template.mean_and_energy())
        .unwrap_or((0.0, 0.0));
    let constant_value = template.constant_value();
    let owned_integral = cached_integral
        .is_none()
        .then(|| GrayFrameIntegral::from_frame(frame));
    let integral = cached_integral.or(owned_integral.as_ref());
    let flat_integral = constant_value.map(|value| {
        (
            integral.expect("candidate scoring integral should exist"),
            value,
        )
    });
    let max_x = frame.width - template.width;
    let max_y = frame.height - template.height;
    let pool_limit = limit
        .saturating_mul(COARSE_CANDIDATE_POOL_MULTIPLIER)
        .max(limit)
        .max(1);
    let mut pool = BinaryHeap::<CandidateLoc>::with_capacity(pool_limit);
    for y in 0..=max_y {
        for x in 0..=max_x {
            let score = if let Some((integral, value)) = &flat_integral {
                integral.flat_difference_score(*value, template.width, template.height, x, y)
            } else if let Some(samples) = &sparse_samples {
                samples.score_at(frame, x, y)
            } else {
                score_template_at(
                    frame,
                    template,
                    template_flat,
                    template_stats,
                    x,
                    y,
                    integral,
                )
            };
            if template_flat && score >= PERFECT_SCORE_THRESHOLD {
                return vec![(x, y)];
            }
            if pool.len() < pool_limit {
                pool.push(CandidateLoc { score, x, y });
                continue;
            }
            if let Some(mut worst) = pool.peek_mut() {
                if score > worst.score {
                    *worst = CandidateLoc { score, x, y };
                }
            }
        }
    }
    let mut pool = pool.into_vec();
    pool.sort_by(|a, b| b.score.total_cmp(&a.score));

    let mut suppressed = Vec::<(u32, u32, u32, u32)>::new();
    let mut locs = Vec::new();
    for CandidateLoc { x, y, .. } in pool {
        if suppressed
            .iter()
            .any(|(x1, y1, x2, y2)| x >= *x1 && x < *x2 && y >= *y1 && y < *y2)
        {
            continue;
        }
        locs.push((x, y));
        if locs.len() >= limit {
            break;
        }
        suppressed.push((
            x.saturating_sub(radius),
            y.saturating_sub(radius),
            (x + radius + 1).min(max_x + 1),
            (y + radius + 1).min(max_y + 1),
        ));
    }
    locs
}

fn refine_template_match(
    frame: &GrayFrame,
    template: &GrayFrame,
    coarse_loc: (u32, u32),
    factor: f64,
    phase_x: u32,
    phase_y: u32,
    cached_integral: Option<&GrayFrameIntegral>,
) -> Option<TemplateMatch> {
    let x = (f64::from(coarse_loc.0) / factor).round() as i64 - i64::from(phase_x);
    let y = (f64::from(coarse_loc.1) / factor).round() as i64 - i64::from(phase_y);
    let coarse_step_margin = ((1.0 / factor).ceil() as u32).saturating_mul(2);
    let margin = REFINE_MARGIN.max(coarse_step_margin);
    let left = (x - i64::from(margin)).max(0) as u32;
    let top = (y - i64::from(margin)).max(0) as u32;
    let right =
        (x + i64::from(template.width) + i64::from(margin)).clamp(0, i64::from(frame.width)) as u32;
    let bottom = (y + i64::from(template.height) + i64::from(margin))
        .clamp(0, i64::from(frame.height)) as u32;
    find_template_best_gray_in_region_cached(
        frame,
        template,
        left,
        top,
        right,
        bottom,
        cached_integral,
    )
}

impl GrayFrame {
    fn from_rgb(frame: &RgbFrame) -> Self {
        let pixels = frame
            .pixels
            .chunks_exact(3)
            .map(|rgb| {
                0.299 * f32::from(rgb[0]) + 0.587 * f32::from(rgb[1]) + 0.114 * f32::from(rgb[2])
            })
            .collect();
        Self {
            width: frame.width,
            height: frame.height,
            pixels,
        }
    }

    fn pixel(&self, x: u32, y: u32) -> f32 {
        self.pixels[(y * self.width + x) as usize]
    }

    fn resize_by(&self, scale: f64) -> Option<Self> {
        if scale <= 0.0 || self.width == 0 || self.height == 0 {
            return None;
        }
        let width = ((f64::from(self.width) * scale) as u32).max(1);
        let height = ((f64::from(self.height) * scale) as u32).max(1);
        let mut pixels = vec![0.0; width as usize * height as usize];
        for y in 0..height {
            for x in 0..width {
                let src_x = ((f64::from(x) / scale).floor() as u32).min(self.width - 1);
                let src_y = ((f64::from(y) / scale).floor() as u32).min(self.height - 1);
                pixels[(y * width + x) as usize] = self.pixel(src_x, src_y);
            }
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    fn resize_area_by(&self, scale: f64) -> Option<Self> {
        if scale <= 0.0 || self.width == 0 || self.height == 0 {
            return None;
        }
        if scale >= 1.0 {
            return self.resize_by(scale);
        }
        let width = ((f64::from(self.width) * scale) as u32).max(1);
        let height = ((f64::from(self.height) * scale) as u32).max(1);
        let mut pixels = vec![0.0; width as usize * height as usize];
        for y in 0..height {
            let source_top = ((f64::from(y) / scale).floor() as u32).min(self.height - 1);
            let source_bottom =
                (((f64::from(y + 1) / scale).ceil() as u32).max(source_top + 1)).min(self.height);
            for x in 0..width {
                let source_left = ((f64::from(x) / scale).floor() as u32).min(self.width - 1);
                let source_right = (((f64::from(x + 1) / scale).ceil() as u32)
                    .max(source_left + 1))
                .min(self.width);
                let mut sum = 0.0;
                let mut count = 0u32;
                for source_y in source_top..source_bottom {
                    for source_x in source_left..source_right {
                        sum += self.pixel(source_x, source_y);
                        count += 1;
                    }
                }
                pixels[(y * width + x) as usize] = sum / count.max(1) as f32;
            }
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    fn mean_and_energy(&self) -> (f32, f32) {
        let count = self.pixels.len() as f32;
        if count == 0.0 {
            return (0.0, 0.0);
        }
        let mean = self.pixels.iter().sum::<f32>() / count;
        let energy = self
            .pixels
            .iter()
            .map(|value| {
                let delta = *value - mean;
                delta * delta
            })
            .sum();
        (mean, energy)
    }

    fn is_flat(&self) -> bool {
        let (_, energy) = self.mean_and_energy();
        let variance = energy / self.pixels.len().max(1) as f32;
        variance.sqrt() < 1.0
    }

    fn constant_value(&self) -> Option<f32> {
        let first = *self.pixels.first()?;
        if self.pixels.iter().all(|value| *value == first) {
            Some(first)
        } else {
            None
        }
    }
}

struct GrayFrameCache {
    full: GrayFrame,
    half: Option<GrayFrame>,
    quarter: Option<GrayFrame>,
    full_integral: OnceLock<GrayFrameIntegral>,
    half_integral: OnceLock<GrayFrameIntegral>,
    quarter_integral: OnceLock<GrayFrameIntegral>,
}

impl GrayFrameCache {
    fn from_rgb(frame: &RgbFrame) -> Self {
        let full = GrayFrame::from_rgb(frame);
        let area = u64::from(full.width) * u64::from(full.height);
        let half = if area >= COARSE_AREA {
            full.resize_area_by(0.5)
        } else {
            None
        };
        let quarter = if area >= QUARTER_AREA {
            full.resize_area_by(0.25)
        } else {
            None
        };
        Self {
            full,
            half,
            quarter,
            full_integral: OnceLock::new(),
            half_integral: OnceLock::new(),
            quarter_integral: OnceLock::new(),
        }
    }

    fn full(&self) -> &GrayFrame {
        &self.full
    }

    fn frame_for_factor(&self, factor: f64) -> Option<&GrayFrame> {
        if (factor - 1.0).abs() < f64::EPSILON {
            Some(&self.full)
        } else if (factor - 0.5).abs() < f64::EPSILON {
            self.half.as_ref()
        } else if (factor - 0.25).abs() < f64::EPSILON {
            self.quarter.as_ref()
        } else {
            None
        }
    }

    fn integral_for_factor(&self, factor: f64) -> Option<&GrayFrameIntegral> {
        if (factor - 1.0).abs() < f64::EPSILON {
            Some(
                self.full_integral
                    .get_or_init(|| GrayFrameIntegral::from_frame(&self.full)),
            )
        } else if (factor - 0.5).abs() < f64::EPSILON {
            let frame = self.half.as_ref()?;
            Some(
                self.half_integral
                    .get_or_init(|| GrayFrameIntegral::from_frame(frame)),
            )
        } else if (factor - 0.25).abs() < f64::EPSILON {
            let frame = self.quarter.as_ref()?;
            Some(
                self.quarter_integral
                    .get_or_init(|| GrayFrameIntegral::from_frame(frame)),
            )
        } else {
            None
        }
    }
}

struct GrayFrameIntegral {
    width: u32,
    sum: Vec<f64>,
    sum_sq: Vec<f64>,
}

impl GrayFrameIntegral {
    fn from_frame(frame: &GrayFrame) -> Self {
        let stride = frame.width as usize + 1;
        let mut sum = vec![0.0; stride * (frame.height as usize + 1)];
        let mut sum_sq = vec![0.0; sum.len()];
        for y in 0..frame.height as usize {
            let mut row_sum = 0.0;
            let mut row_sum_sq = 0.0;
            for x in 0..frame.width as usize {
                let value = f64::from(frame.pixels[y * frame.width as usize + x]);
                row_sum += value;
                row_sum_sq += value * value;
                let index = (y + 1) * stride + x + 1;
                sum[index] = sum[y * stride + x + 1] + row_sum;
                sum_sq[index] = sum_sq[y * stride + x + 1] + row_sum_sq;
            }
        }
        Self {
            width: frame.width,
            sum,
            sum_sq,
        }
    }

    fn flat_difference_score(
        &self,
        template_value: f32,
        template_width: u32,
        template_height: u32,
        left: u32,
        top: u32,
    ) -> f32 {
        let count = f64::from(template_width * template_height);
        let sum = self.region_sum(&self.sum, left, top, template_width, template_height);
        let sum_sq = self.region_sum(&self.sum_sq, left, top, template_width, template_height);
        let value = f64::from(template_value);
        let mse = (sum_sq - 2.0 * value * sum + count * value * value) / count;
        let rmse = mse.max(0.0).sqrt();
        (1.0 - (rmse / 255.0) as f32).clamp(0.0, 1.0)
    }

    fn window_mean_and_energy(&self, left: u32, top: u32, width: u32, height: u32) -> (f32, f32) {
        let count = f64::from(width * height);
        let sum = self.region_sum(&self.sum, left, top, width, height);
        let sum_sq = self.region_sum(&self.sum_sq, left, top, width, height);
        let mean = sum / count;
        let energy = (sum_sq - (sum * sum / count)).max(0.0);
        (mean as f32, energy as f32)
    }

    fn region_sum(&self, integral: &[f64], left: u32, top: u32, width: u32, height: u32) -> f64 {
        let stride = self.width as usize + 1;
        let x1 = left as usize;
        let y1 = top as usize;
        let x2 = (left + width) as usize;
        let y2 = (top + height) as usize;
        integral[y2 * stride + x2] + integral[y1 * stride + x1]
            - integral[y1 * stride + x2]
            - integral[y2 * stride + x1]
    }
}

fn find_template_best_gray_in_region_cached(
    frame: &GrayFrame,
    template: &GrayFrame,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    cached_integral: Option<&GrayFrameIntegral>,
) -> Option<TemplateMatch> {
    if template.width == 0
        || template.height == 0
        || template.width > frame.width
        || template.height > frame.height
        || right > frame.width
        || bottom > frame.height
        || right < left.saturating_add(template.width)
        || bottom < top.saturating_add(template.height)
    {
        return None;
    }
    let template_flat = template.is_flat();
    let template_stats = template.mean_and_energy();
    let constant_value = template.constant_value();
    let owned_integral = cached_integral
        .is_none()
        .then(|| GrayFrameIntegral::from_frame(frame));
    let integral = cached_integral.or(owned_integral.as_ref());
    let flat_integral = constant_value.map(|value| {
        (
            integral.expect("template scoring integral should exist"),
            value,
        )
    });
    let mut best: Option<TemplateMatch> = None;
    let max_x = right - template.width;
    let max_y = bottom - template.height;
    for y in top..=max_y {
        for x in left..=max_x {
            let score = if let Some((integral, value)) = &flat_integral {
                integral.flat_difference_score(*value, template.width, template.height, x, y)
            } else {
                score_template_at(
                    frame,
                    template,
                    template_flat,
                    template_stats,
                    x,
                    y,
                    integral,
                )
            };
            if score >= PERFECT_SCORE_THRESHOLD {
                return Some(TemplateMatch {
                    score,
                    box_xyxy: [x, y, x + template.width, y + template.height],
                    scale: 1.0,
                });
            }
            if best
                .as_ref()
                .map(|current| score > current.score)
                .unwrap_or(true)
            {
                best = Some(TemplateMatch {
                    score,
                    box_xyxy: [x, y, x + template.width, y + template.height],
                    scale: 1.0,
                });
            }
        }
    }
    best
}

fn score_template_at(
    frame: &GrayFrame,
    template: &GrayFrame,
    template_flat: bool,
    template_stats: (f32, f32),
    left: u32,
    top: u32,
    integral: Option<&GrayFrameIntegral>,
) -> f32 {
    if template_flat {
        flat_difference_score(frame, template, left, top)
    } else {
        normalized_cross_correlation(frame, template, template_stats, left, top, integral)
    }
}

fn flat_difference_score(frame: &GrayFrame, template: &GrayFrame, left: u32, top: u32) -> f32 {
    let mut sum_sq = 0.0;
    let count = (template.width * template.height) as f32;
    for ty in 0..template.height {
        for tx in 0..template.width {
            let delta = frame.pixel(left + tx, top + ty) - template.pixel(tx, ty);
            sum_sq += delta * delta;
        }
    }
    let rmse = (sum_sq / count).sqrt();
    (1.0 - rmse / 255.0).clamp(0.0, 1.0)
}

fn normalized_cross_correlation(
    frame: &GrayFrame,
    template: &GrayFrame,
    template_stats: (f32, f32),
    left: u32,
    top: u32,
    integral: Option<&GrayFrameIntegral>,
) -> f32 {
    let (template_mean, template_energy) = template_stats;
    if template_energy <= f32::EPSILON {
        return flat_difference_score(frame, template, left, top);
    }
    let (window_mean, window_energy) = if let Some(integral) = integral {
        integral.window_mean_and_energy(left, top, template.width, template.height)
    } else {
        let count = (template.width * template.height) as f32;
        let mut window_sum = 0.0;
        for ty in 0..template.height {
            for tx in 0..template.width {
                window_sum += frame.pixel(left + tx, top + ty);
            }
        }
        let window_mean = window_sum / count;
        let mut window_energy = 0.0;
        for ty in 0..template.height {
            for tx in 0..template.width {
                let window_delta = frame.pixel(left + tx, top + ty) - window_mean;
                window_energy += window_delta * window_delta;
            }
        }
        (window_mean, window_energy)
    };
    let mut numerator = 0.0;
    for ty in 0..template.height {
        for tx in 0..template.width {
            let window_delta = frame.pixel(left + tx, top + ty) - window_mean;
            let template_delta = template.pixel(tx, ty) - template_mean;
            numerator += window_delta * template_delta;
        }
    }
    if window_energy <= f32::EPSILON {
        return flat_difference_score(frame, template, left, top);
    }
    (numerator / (window_energy.sqrt() * template_energy.sqrt())).clamp(-1.0, 1.0)
}

fn resolve_path(base_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn template_decode_error(path: &Path, err: png::DecodingError) -> DetectError {
    DetectError::TemplateDecode {
        path: path.to_path_buf(),
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        detect_ocr_text_targets, detect_pixel, detect_targets, detect_template_scaled,
        find_template_exact, find_template_scaled, template_worker_count, DetectError, OcrTextRow,
        PreparedDetector, RgbFrame,
    };
    use crate::config::WatchConfig;
    use std::{fs::File, io::Cursor, path::Path};

    #[test]
    fn pixel_detection_matches_with_tolerance() {
        let frame = RgbFrame::new(3, 3, vec![0; 3 * 3 * 3]).unwrap();
        assert!(detect_pixel(&frame, "black", Some("id"), 1, 1, [0, 0, 0], 0).is_some());
        assert!(detect_pixel(&frame, "red", None, 1, 1, [255, 0, 0], 12).is_none());
    }

    #[test]
    fn detect_targets_keeps_target_identity() {
        let text = r#"{
          "regions": [{"name":"screen","monitor":1}],
          "targets": [{"kind":"pixel","id":"stable-id","name":"red-dot","x":1,"y":0,"rgb":[9,8,7],"tolerance":0}]
        }"#;
        let config = WatchConfig::from_json_str(text).unwrap();
        let frame = RgbFrame::new(2, 1, vec![0, 0, 0, 9, 8, 7]).unwrap();
        let matches = detect_targets(&frame, &config.targets);
        assert_eq!(matches[0].target_id, "stable-id");
    }

    #[test]
    fn ocr_text_detection_matches_case_insensitive_text_score_and_box() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"ocr_text","id":"ocr-id","name":"alert-text","text":"alert","min_score":0.5}
              ]
            }"#,
        )
        .unwrap();
        let rows = vec![
            ocr_row("quiet", 0.99, Some(vec![[1.0, 1.0], [3.0, 1.0]])),
            ocr_row(
                "ALERT-42",
                0.8,
                Some(vec![[10.2, 20.8], [30.9, 20.1], [30.4, 40.9], [10.0, 40.0]]),
            ),
        ];

        let matches = detect_ocr_text_targets(&rows, &config.targets);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].target_id, "ocr-id");
        assert_eq!(matches[0].kind, "ocr_text");
        assert_eq!(matches[0].score, 0.8);
        assert_eq!(matches[0].text.as_deref(), Some("ALERT-42"));
        assert_eq!(matches[0].box_xyxy, [10, 20, 30, 40]);
    }

    #[test]
    fn ocr_text_detection_respects_min_score_and_case_sensitive_flag() {
        let rows = vec![ocr_row("ALERT", 0.7, None)];
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"ocr_text","name":"too-strict","text":"ALERT","min_score":0.8},
                {"kind":"ocr_text","name":"case-miss","text":"alert","min_score":0.1,"case_sensitive":true},
                {"kind":"ocr_text","name":"case-hit","text":"ALERT","min_score":0.1,"case_sensitive":true}
              ]
            }"#,
        )
        .unwrap();

        let matches = detect_ocr_text_targets(&rows, &config.targets);

        assert_eq!(
            matches
                .iter()
                .map(|item| item.target.as_str())
                .collect::<Vec<_>>(),
            vec!["case-hit"]
        );
        assert_eq!(matches[0].box_xyxy, [0, 0, 0, 0]);
    }

    #[test]
    fn ocr_text_detection_matches_unicode_contains() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"ocr_text","id":"zh-id","name":"zh-ready","text":"准备","min_score":0.5}
              ]
            }"#,
        )
        .unwrap();
        let rows = vec![ocr_row(
            "准备好了",
            0.88,
            Some(vec![[5.0, 6.0], [45.0, 6.0], [45.0, 26.0], [5.0, 26.0]]),
        )];

        let matches = detect_ocr_text_targets(&rows, &config.targets);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].target_id, "zh-id");
        assert_eq!(matches[0].text.as_deref(), Some("准备好了"));
        assert_eq!(matches[0].box_xyxy, [5, 6, 45, 26]);
    }

    #[test]
    fn exact_template_detection_finds_box() {
        let frame = RgbFrame::new(
            4,
            3,
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
                0, 0, 0, 1, 2, 3, 4, 5, 6, 0, 0, 0, //
                0, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0, 0,
            ],
        )
        .unwrap();
        let template = RgbFrame::new(2, 2, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]).unwrap();
        assert_eq!(find_template_exact(&frame, &template), Some([1, 1, 3, 3]));
    }

    #[test]
    fn exact_gray_template_detection_uses_rarest_anchor_and_checks_all_columns() {
        let template = super::GrayFrame {
            width: 3,
            height: 3,
            pixels: vec![
                10.0, 10.0, 10.0, //
                10.0, 10.0, 99.0, //
                10.0, 10.0, 10.0,
            ],
        };
        let anchor = super::exact_match_anchor(&template).unwrap();
        assert_eq!(
            anchor,
            super::ExactGrayAnchor {
                x: 2,
                y: 1,
                value: 99.0
            }
        );

        let mut pixels = vec![1.0; 8 * 6];
        for y in 0..template.height {
            for x in 0..template.width {
                pixels[((y + 2) * 8 + x + 4) as usize] = template.pixel(x, y);
            }
        }
        let frame = super::GrayFrame {
            width: 8,
            height: 6,
            pixels,
        };

        let hit = super::find_template_exact_gray(&frame, &template).unwrap();

        assert_eq!(hit.box_xyxy, [4, 2, 7, 5]);
        assert_eq!(hit.score, 1.0);
    }

    #[test]
    fn textured_template_detection_returns_best_box_and_score() {
        let mut pixels = vec![0; 8 * 8 * 3];
        let template = RgbFrame::new(
            3,
            3,
            vec![
                20, 20, 20, 80, 80, 80, 140, 140, 140, //
                40, 40, 40, 100, 100, 100, 160, 160, 160, //
                60, 60, 60, 120, 120, 120, 180, 180, 180,
            ],
        )
        .unwrap();
        paste(&mut pixels, 8, 2, 4, &template);
        let frame = RgbFrame::new(8, 8, pixels).unwrap();
        let hit = find_template_scaled(&frame, &template, &[1.0]).unwrap();
        assert_eq!(hit.box_xyxy, [2, 4, 5, 7]);
        assert!(hit.score > 0.999);
    }

    #[test]
    fn scaled_template_detection_uses_python_style_floor_dimensions() {
        let template = RgbFrame::new(
            4,
            4,
            vec![
                10, 10, 10, 30, 30, 30, 50, 50, 50, 70, 70, 70, //
                20, 20, 20, 40, 40, 40, 60, 60, 60, 80, 80, 80, //
                90, 90, 90, 110, 110, 110, 130, 130, 130, 150, 150, 150, //
                100, 100, 100, 120, 120, 120, 140, 140, 140, 160, 160, 160,
            ],
        )
        .unwrap();
        let scaled = super::GrayFrame::from_rgb(&template)
            .resize_by(1.5)
            .unwrap();
        assert_eq!((scaled.width, scaled.height), (6, 6));

        let mut pixels = vec![0; 12 * 12 * 3];
        let scaled_rgb = gray_to_rgb(&scaled);
        paste(&mut pixels, 12, 5, 3, &scaled_rgb);
        let frame = RgbFrame::new(12, 12, pixels).unwrap();
        let hit =
            detect_template_scaled(&frame, "target", Some("id"), &template, 0.99, &[1.0, 1.5])
                .unwrap();
        assert_eq!(hit.target_id, "id");
        assert_eq!(hit.box_xyxy, [5, 3, 11, 9]);
        assert_eq!(hit.scale, Some(1.5));
    }

    #[test]
    fn large_frame_template_detection_uses_coarse_refine_without_missing_unaligned_hit() {
        let template = textured_template(6, 6);
        let left = 777;
        let top = 555;
        let mut pixels = vec![5; 2560 * 1440 * 3];
        paste(&mut pixels, 2560, left, top, &template);
        let frame = RgbFrame::new(2560, 1440, pixels).unwrap();

        let hit = find_template_scaled(&frame, &template, &[1.0]).unwrap();

        assert_eq!(hit.box_xyxy, [left, top, left + 6, top + 6]);
        assert!(hit.score > 0.999);
    }

    #[test]
    fn coarse_plan_skips_textured_templates_when_downscale_loses_detail() {
        let frame = super::GrayFrame {
            width: 2560,
            height: 1440,
            pixels: Vec::new(),
        };
        let mut pixels = Vec::new();
        for y in 0..6 {
            for x in 0..6 {
                pixels.push(if (x + y) % 2 == 0 { 0.0 } else { 255.0 });
            }
        }
        let template = super::GrayFrame {
            width: 6,
            height: 6,
            pixels,
        };

        assert!(super::coarse_plan(&frame, &template).is_none());
    }

    #[test]
    fn exact_gray_search_skips_flat_templates_and_bounds_large_scans() {
        let small_frame = gray_frame(400, 300, 0.0);
        let large_frame = gray_frame(1280, 720, 0.0);
        let textured = super::GrayFrame::from_rgb(&textured_template(12, 12));
        let flat = gray_frame(12, 12, 90.0);

        assert!(super::should_try_exact_gray(&small_frame, &textured));
        assert!(super::should_try_exact_gray(&large_frame, &textured));
        assert!(super::find_template_exact_gray_bounded(&large_frame, &textured, 1).is_none());
        assert!(!super::should_try_exact_gray(&small_frame, &flat));
    }

    #[test]
    fn flat_template_uses_difference_score_instead_of_correlation() {
        let template = RgbFrame::new(2, 2, vec![90; 2 * 2 * 3]).unwrap();
        let mut pixels = vec![0; 5 * 5 * 3];
        paste(&mut pixels, 5, 3, 1, &template);
        let frame = RgbFrame::new(5, 5, pixels).unwrap();
        let hit = find_template_scaled(&frame, &template, &[1.0]).unwrap();
        assert_eq!(hit.box_xyxy, [3, 1, 5, 3]);
        assert_eq!(hit.score, 1.0);
    }

    #[test]
    fn gray_frame_integral_score_matches_pixel_difference_score() {
        let frame = super::GrayFrame {
            width: 4,
            height: 3,
            pixels: vec![
                10.0, 20.0, 30.0, 40.0, //
                50.0, 60.0, 70.0, 80.0, //
                90.0, 100.0, 110.0, 120.0,
            ],
        };
        let template = super::GrayFrame {
            width: 2,
            height: 2,
            pixels: vec![60.0; 4],
        };
        let integral = super::GrayFrameIntegral::from_frame(&frame);

        assert_eq!(
            integral.flat_difference_score(60.0, 2, 2, 1, 1),
            super::flat_difference_score(&frame, &template, 1, 1)
        );
    }

    #[test]
    fn gray_frame_integral_ncc_matches_uncached_window_stats() {
        let frame = super::GrayFrame {
            width: 4,
            height: 3,
            pixels: vec![
                10.0, 20.0, 30.0, 40.0, //
                50.0, 60.0, 70.0, 80.0, //
                90.0, 100.0, 110.0, 120.0,
            ],
        };
        let template = super::GrayFrame {
            width: 2,
            height: 2,
            pixels: vec![20.0, 30.0, 60.0, 70.0],
        };
        let stats = template.mean_and_energy();
        let integral = super::GrayFrameIntegral::from_frame(&frame);

        let uncached = super::normalized_cross_correlation(&frame, &template, stats, 1, 0, None);
        let cached =
            super::normalized_cross_correlation(&frame, &template, stats, 1, 0, Some(&integral));

        assert!((cached - uncached).abs() < 1e-6);
    }

    #[test]
    fn sparse_texture_candidate_score_prefers_exact_window() {
        let template = super::GrayFrame {
            width: 3,
            height: 3,
            pixels: vec![
                20.0, 80.0, 140.0, //
                40.0, 100.0, 160.0, //
                60.0, 120.0, 180.0,
            ],
        };
        let mut pixels = vec![0.0; 5 * 5];
        for y in 0..template.height {
            for x in 0..template.width {
                pixels[((y + 1) * 5 + x + 1) as usize] = template.pixel(x, y);
            }
        }
        let frame = super::GrayFrame {
            width: 5,
            height: 5,
            pixels,
        };
        let samples = super::SparseTemplateSamples::from_template(&template).unwrap();

        let exact = samples.score_at(&frame, 1, 1);
        let shifted = samples.score_at(&frame, 0, 0);

        assert!(exact > 0.999);
        assert!(shifted < exact);
    }

    #[test]
    fn template_threshold_filters_weak_matches() {
        let template = RgbFrame::new(2, 2, vec![200; 2 * 2 * 3]).unwrap();
        let frame = RgbFrame::new(3, 3, vec![0; 3 * 3 * 3]).unwrap();
        assert!(detect_template_scaled(&frame, "target", None, &template, 0.95, &[1.0]).is_none());
    }

    #[test]
    fn prepared_detector_loads_relative_png_templates_and_preserves_target_order() {
        let tmp = tempfile::tempdir().unwrap();
        let template = RgbFrame::new(
            2,
            2,
            vec![
                5, 5, 5, 25, 25, 25, //
                45, 45, 45, 65, 65, 65,
            ],
        )
        .unwrap();
        write_png(&tmp.path().join("target.png"), &template);

        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"pixel","id":"pixel-id","name":"pixel","x":0,"y":0,"rgb":[9,8,7],"tolerance":0},
                {"kind":"template","id":"template-id","name":"template","path":"target.png","threshold":0.99,"scales":[1.0]}
              ]
            }"#,
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let mut pixels = vec![0; 5 * 5 * 3];
        pixels[0..3].copy_from_slice(&[9, 8, 7]);
        paste(&mut pixels, 5, 3, 2, &template);
        let frame = RgbFrame::new(5, 5, pixels).unwrap();
        let matches = detector.run(&frame);
        assert_eq!(
            matches
                .iter()
                .map(|item| item.target_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pixel-id", "template-id"]
        );
        assert_eq!(matches[1].box_xyxy, [3, 2, 5, 4]);
    }

    #[test]
    fn prepared_detector_loads_common_image_template_formats() {
        let tmp = tempfile::tempdir().unwrap();
        let template = RgbFrame::new(
            2,
            2,
            vec![
                10, 20, 30, 40, 50, 60, //
                70, 80, 90, 100, 110, 120,
            ],
        )
        .unwrap();
        write_image(
            &tmp.path().join("target.bmp"),
            &template,
            image::ImageFormat::Bmp,
        );

        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"template","id":"template-id","name":"template","path":"target.bmp","threshold":0.99,"scales":[1.0]}
              ]
            }"#,
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let mut pixels = vec![0; 5 * 5 * 3];
        paste(&mut pixels, 5, 2, 1, &template);
        let frame = RgbFrame::new(5, 5, pixels).unwrap();

        let matches = detector.run(&frame);

        assert_eq!(matches[0].target_id, "template-id");
        assert_eq!(matches[0].box_xyxy, [2, 1, 4, 3]);
    }

    #[test]
    fn prepared_detector_uses_configured_template_worker_limit_and_keeps_order() {
        let tmp = tempfile::tempdir().unwrap();
        let one = RgbFrame::new(2, 2, vec![80; 2 * 2 * 3]).unwrap();
        let two = RgbFrame::new(2, 2, vec![180; 2 * 2 * 3]).unwrap();
        write_png(&tmp.path().join("one.png"), &one);
        write_png(&tmp.path().join("two.png"), &two);
        let config = WatchConfig::from_json_str(
            r#"{
              "template_workers": 2,
              "targets": [
                {"kind":"template","id":"one-id","name":"one","path":"one.png","threshold":0.99,"scales":[1.0]},
                {"kind":"template","id":"two-id","name":"two","path":"two.png","threshold":0.99,"scales":[1.0]},
                {"kind":"pixel","id":"pixel-id","name":"pixel","x":0,"y":0,"rgb":[9,8,7],"tolerance":0}
              ]
            }"#,
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let mut pixels = vec![0; 8 * 5 * 3];
        pixels[0..3].copy_from_slice(&[9, 8, 7]);
        paste(&mut pixels, 8, 1, 1, &one);
        paste(&mut pixels, 8, 5, 2, &two);
        let frame = RgbFrame::new(8, 5, pixels).unwrap();

        let matches = detector.run(&frame);

        assert_eq!(detector.template_worker_limit(), 2);
        assert_eq!(
            template_worker_count(detector.template_worker_limit(), 2),
            2
        );
        assert_eq!(
            matches
                .iter()
                .map(|item| item.target_id.as_str())
                .collect::<Vec<_>>(),
            vec!["one-id", "two-id", "pixel-id"]
        );
        assert_eq!(matches[0].box_xyxy, [1, 1, 3, 3]);
        assert_eq!(matches[1].box_xyxy, [5, 2, 7, 4]);
    }

    #[test]
    fn template_worker_count_caps_to_jobs_and_clamps_zero_limit() {
        assert_eq!(template_worker_count(8, 0), 0);
        assert_eq!(template_worker_count(8, 1), 1);
        assert_eq!(template_worker_count(8, 2), 2);
        assert_eq!(template_worker_count(2, 8), 2);
        assert_eq!(template_worker_count(0, 3), 1);
    }

    #[test]
    #[ignore = "benchmark gate; run through scripts\\template-benchmark.ps1"]
    fn benchmark_large_frame_many_template_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let frame_width = 2560;
        let frame_height = 1440;
        let template_count = 8usize;
        let mut frame_pixels = vec![3u8; frame_width as usize * frame_height as usize * 3];
        let mut targets = Vec::new();
        let mut expected_boxes = Vec::new();

        for index in 0..template_count {
            let value = 40 + index as u8 * 23;
            let template = flat_template(12, 12, value);
            let file_name = format!("target-{index}.png");
            write_png(&tmp.path().join(&file_name), &template);
            let left = 137 + index as u32 * 211;
            let top = 193 + index as u32 * 97;
            paste(&mut frame_pixels, frame_width, left, top, &template);
            expected_boxes.push([left, top, left + template.width, top + template.height]);
            targets.push(serde_json::json!({
                "kind": "template",
                "id": format!("target-{index}"),
                "name": format!("target-{index}"),
                "path": file_name,
                "threshold": 0.99,
                "scales": [1.0],
            }));
        }

        let config = WatchConfig::from_json_str(
            &serde_json::json!({
                "template_workers": 4,
                "targets": targets,
            })
            .to_string(),
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let frame = RgbFrame::new(frame_width, frame_height, frame_pixels).unwrap();
        let started = std::time::Instant::now();

        let matches = detector.run(&frame);
        let elapsed = started.elapsed();

        println!(
            "templateBenchmarkMs={} frame={}x{} templates={} workers={} matches={}",
            elapsed.as_millis(),
            frame_width,
            frame_height,
            template_count,
            detector.template_worker_limit(),
            matches.len()
        );
        assert_eq!(matches.len(), template_count);
        for (index, item) in matches.iter().enumerate() {
            assert_eq!(item.target_id, format!("target-{index}"));
            assert_eq!(item.box_xyxy, expected_boxes[index]);
            assert!(item.score >= 0.99);
        }

        if let Ok(max_ms) = std::env::var("SCREENWATCH_TEMPLATE_BENCH_MAX_MS") {
            let max_ms = max_ms
                .parse::<u128>()
                .expect("SCREENWATCH_TEMPLATE_BENCH_MAX_MS must be an integer");
            assert!(
                elapsed.as_millis() <= max_ms,
                "template benchmark took {}ms, above {}ms",
                elapsed.as_millis(),
                max_ms
            );
        }
    }

    #[test]
    #[ignore = "benchmark gate; run through scripts\\template-benchmark.ps1"]
    fn benchmark_large_frame_textured_template_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let frame_width = 2560;
        let frame_height = 1440;
        let template_count = 8usize;
        let mut frame_pixels = vec![7u8; frame_width as usize * frame_height as usize * 3];
        let mut targets = Vec::new();
        let mut expected_boxes = Vec::new();

        for index in 0..template_count {
            let template = seeded_textured_template(12, 12, index as u32 + 1);
            let file_name = format!("textured-target-{index}.png");
            write_png(&tmp.path().join(&file_name), &template);
            let left = 149 + index as u32 * 223;
            let top = 211 + index as u32 * 101;
            paste(&mut frame_pixels, frame_width, left, top, &template);
            expected_boxes.push([left, top, left + template.width, top + template.height]);
            targets.push(serde_json::json!({
                "kind": "template",
                "id": format!("textured-target-{index}"),
                "name": format!("textured-target-{index}"),
                "path": file_name,
                "threshold": 0.99,
                "scales": [1.0],
            }));
        }

        let config = WatchConfig::from_json_str(
            &serde_json::json!({
                "template_workers": 4,
                "targets": targets,
            })
            .to_string(),
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let frame = RgbFrame::new(frame_width, frame_height, frame_pixels).unwrap();
        let started = std::time::Instant::now();

        let matches = detector.run(&frame);
        let elapsed = started.elapsed();

        println!(
            "texturedTemplateBenchmarkMs={} frame={}x{} templates={} workers={} matches={}",
            elapsed.as_millis(),
            frame_width,
            frame_height,
            template_count,
            detector.template_worker_limit(),
            matches.len()
        );
        assert_eq!(matches.len(), template_count);
        for (index, item) in matches.iter().enumerate() {
            assert_eq!(item.target_id, format!("textured-target-{index}"));
            assert_eq!(item.box_xyxy, expected_boxes[index]);
            assert!(item.score >= 0.99);
        }

        if let Ok(max_ms) = std::env::var("SCREENWATCH_TEMPLATE_BENCH_MAX_MS") {
            let max_ms = max_ms
                .parse::<u128>()
                .expect("SCREENWATCH_TEMPLATE_BENCH_MAX_MS must be an integer");
            assert!(
                elapsed.as_millis() <= max_ms,
                "textured template benchmark took {}ms, above {}ms",
                elapsed.as_millis(),
                max_ms
            );
        }
    }

    #[test]
    fn prepared_detector_merges_external_ocr_rows_in_config_order() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"ocr_text","id":"ocr-id","name":"ocr","text":"READY","min_score":0.4},
                {"kind":"pixel","id":"pixel-id","name":"pixel","x":0,"y":0,"rgb":[9,8,7],"tolerance":0}
              ]
            }"#,
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, std::env::temp_dir()).unwrap();
        let frame = RgbFrame::new(1, 1, vec![9, 8, 7]).unwrap();
        assert_eq!(
            detector
                .run(&frame)
                .iter()
                .map(|item| item.target_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pixel-id"]
        );

        let matches = detector.run_with_ocr_rows(&frame, &[ocr_row("READY", 0.9, None)]);

        assert_eq!(
            matches
                .iter()
                .map(|item| item.target_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ocr-id", "pixel-id"]
        );
    }

    #[test]
    fn prepared_detector_supports_scaled_template_config_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let template = RgbFrame::new(
            4,
            4,
            vec![
                10, 10, 10, 30, 30, 30, 50, 50, 50, 70, 70, 70, //
                20, 20, 20, 40, 40, 40, 60, 60, 60, 80, 80, 80, //
                90, 90, 90, 110, 110, 110, 130, 130, 130, 150, 150, 150, //
                100, 100, 100, 120, 120, 120, 140, 140, 140, 160, 160, 160,
            ],
        )
        .unwrap();
        write_png(&tmp.path().join("target.png"), &template);
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"template","name":"template","path":"target.png","threshold":0.99,"scales":"1.0-1.5:0.5"}
              ]
            }"#,
        )
        .unwrap();
        let detector = PreparedDetector::from_config(&config, tmp.path()).unwrap();
        let scaled = super::GrayFrame::from_rgb(&template)
            .resize_by(1.5)
            .unwrap();
        let scaled_rgb = gray_to_rgb(&scaled);
        let mut pixels = vec![0; 12 * 12 * 3];
        paste(&mut pixels, 12, 5, 3, &scaled_rgb);
        let matches = detector.run(&RgbFrame::new(12, 12, pixels).unwrap());
        assert_eq!(matches[0].scale, Some(1.5));
        assert_eq!(matches[0].box_xyxy, [5, 3, 11, 9]);
    }

    #[test]
    fn prepared_detector_reports_missing_template_path() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [
                {"kind":"template","name":"template","path":"missing.png","threshold":0.99,"scales":[1.0]}
              ]
            }"#,
        )
        .unwrap();
        let err = PreparedDetector::from_config(&config, tmp.path()).unwrap_err();
        assert!(matches!(err, DetectError::TemplateIo { .. }));
    }

    #[test]
    fn png_loader_accepts_rgba_and_grayscale_templates() {
        let tmp = tempfile::tempdir().unwrap();
        write_png_raw(
            &tmp.path().join("rgba.png"),
            1,
            1,
            png::ColorType::Rgba,
            &[10, 20, 30, 40],
        );
        write_png_raw(
            &tmp.path().join("gray.png"),
            1,
            1,
            png::ColorType::Grayscale,
            &[70],
        );
        assert_eq!(
            RgbFrame::from_png_path(tmp.path().join("rgba.png"))
                .unwrap()
                .pixels,
            vec![10, 20, 30]
        );
        assert_eq!(
            RgbFrame::from_png_path(tmp.path().join("gray.png"))
                .unwrap()
                .pixels,
            vec![70, 70, 70]
        );
    }

    #[test]
    fn image_bytes_loader_accepts_clipboard_png_bytes() {
        let expected = RgbFrame::new(
            2,
            1,
            vec![
                10, 20, 30, //
                70, 80, 90,
            ],
        )
        .unwrap();
        let png = image_bytes(&expected, image::ImageFormat::Png);

        let frame = RgbFrame::from_image_bytes("clipboard.png", &png).unwrap();

        assert_eq!(frame, expected);
    }

    fn paste(frame: &mut [u8], frame_width: u32, left: u32, top: u32, image: &RgbFrame) {
        for y in 0..image.height {
            for x in 0..image.width {
                let target = (((top + y) * frame_width + left + x) * 3) as usize;
                let source = ((y * image.width + x) * 3) as usize;
                frame[target..target + 3].copy_from_slice(&image.pixels[source..source + 3]);
            }
        }
    }

    fn gray_to_rgb(gray: &super::GrayFrame) -> RgbFrame {
        let mut pixels = Vec::with_capacity(gray.pixels.len() * 3);
        for value in &gray.pixels {
            let value = value.round().clamp(0.0, 255.0) as u8;
            pixels.extend([value, value, value]);
        }
        RgbFrame::new(gray.width, gray.height, pixels).unwrap()
    }

    fn textured_template(width: u32, height: u32) -> RgbFrame {
        seeded_textured_template(width, height, 0)
    }

    fn seeded_textured_template(width: u32, height: u32, seed: u32) -> RgbFrame {
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.extend([
                    30 + ((x * 37 + y * 11 + seed * 17) % 190) as u8,
                    20 + ((x * 13 + y * 41 + seed * 29) % 200) as u8,
                    40 + ((x * 23 + y * 29 + seed * 31) % 180) as u8,
                ]);
            }
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn flat_template(width: u32, height: u32, value: u8) -> RgbFrame {
        RgbFrame::new(
            width,
            height,
            vec![value; width as usize * height as usize * 3],
        )
        .unwrap()
    }

    fn gray_frame(width: u32, height: u32, value: f32) -> super::GrayFrame {
        super::GrayFrame {
            width,
            height,
            pixels: vec![value; width as usize * height as usize],
        }
    }

    fn write_png(path: &Path, image: &RgbFrame) {
        write_png_raw(
            path,
            image.width,
            image.height,
            png::ColorType::Rgb,
            &image.pixels,
        );
    }

    fn write_image(path: &Path, image: &RgbFrame, format: image::ImageFormat) {
        let image =
            image::RgbImage::from_raw(image.width, image.height, image.pixels.clone()).unwrap();
        image.save_with_format(path, format).unwrap();
    }

    fn image_bytes(image: &RgbFrame, format: image::ImageFormat) -> Vec<u8> {
        let image =
            image::RgbImage::from_raw(image.width, image.height, image.pixels.clone()).unwrap();
        let mut cursor = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, format)
            .unwrap();
        cursor.into_inner()
    }

    fn write_png_raw(
        path: &Path,
        width: u32,
        height: u32,
        color_type: png::ColorType,
        pixels: &[u8],
    ) {
        let file = File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
    }

    fn ocr_row(text: &str, score: f32, box_points: Option<Vec<[f32; 2]>>) -> OcrTextRow {
        OcrTextRow {
            text: text.to_string(),
            score,
            box_points,
        }
    }
}
