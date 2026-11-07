use std::{
    fmt, fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use image::{
    AnimationDecoder, Frame, ImageDecoder, RgbaImage,
    codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder},
    imageops::{self, FilterType},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use webp_animation::{AnimParams, Encoder, EncoderOptions, EncodingConfig};

const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PRESERVED_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FRAMES: u32 = 240;
const MAX_DURATION_MS: u64 = 15_000;
const MIN_FRAME_INTERVAL_MS: u64 = 50;
const MAX_OUTPUT_DIMENSION: u32 = 1920;
const MAX_PRESERVE_DIMENSION: u32 = 3840;
const PRESERVE_MAX_FRAMES: u32 = 1_000;
const PRESERVE_MAX_DURATION_MS: u64 = 60_000;
const DEFAULT_QUALITY: f32 = 75.0;
const PRESERVE_QUALITY: f32 = 90.0;

static CONVERSION_PERMIT: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

fn conversion_permit() -> &'static tokio::sync::Semaphore {
    CONVERSION_PERMIT.get_or_init(|| tokio::sync::Semaphore::new(1))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundConversionError {
    pub code: String,
    pub message: String,
}

impl BackgroundConversionError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl fmt::Display for BackgroundConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BackgroundConversionError {}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCropRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConversionBackend {
    Auto,
    Native,
    Ffmpeg,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimatedBackgroundOptions {
    pub input_path: String,
    pub output_path: String,
    pub crop: NormalizedCropRect,
    pub output_width: u32,
    pub output_height: u32,
    pub quality: Option<f32>,
    pub max_frames: Option<u32>,
    pub max_duration_ms: Option<u64>,
    pub max_output_bytes: Option<u64>,
    pub allow_oversized_source: Option<bool>,
    pub preserve_original_quality: Option<bool>,
    pub backend: Option<ConversionBackend>,
    pub ffmpeg_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimatedBackgroundResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub duration_ms: u64,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimatedFormat {
    Gif,
    Apng,
    AnimatedWebp,
    Avif,
    Mp4,
}

impl AnimatedFormat {
    fn is_native_supported(self) -> bool {
        matches!(self, Self::Gif | Self::Apng | Self::AnimatedWebp)
    }
}

#[derive(Debug)]
struct ConversionOutput {
    width: u32,
    height: u32,
    frame_count: u32,
    duration_ms: u64,
    bytes: Vec<u8>,
}

#[tauri::command]
pub async fn background_convert_animated<R: Runtime>(
    app: AppHandle<R>,
    options: AnimatedBackgroundOptions,
) -> Result<AnimatedBackgroundResult, BackgroundConversionError> {
    if !is_valid_crop(&options.crop) {
        return Err(BackgroundConversionError::new(
            "invalid_crop",
            "The requested crop rectangle is invalid",
        ));
    }

    let input_path = PathBuf::from(&options.input_path);
    let output_path = PathBuf::from(&options.output_path);
    validate_output_path(&app, &output_path)?;

    let permit = conversion_permit().acquire().await.map_err(|_| {
        BackgroundConversionError::new("cancelled", "Conversion was cancelled")
    })?;

    let result = tokio::task::spawn_blocking(move || {
        convert_background(&input_path, &options)
    })
    .await
    .map_err(|error| {
        BackgroundConversionError::new(
            "conversion_failed",
            format!("The conversion task failed: {error}"),
        )
    })?
    .map_err(|error| {
        drop(permit);
        error
    })?;

    let byte_length = result.bytes.len() as u64;
    write_atomically(&output_path, &result.bytes)?;

    Ok(AnimatedBackgroundResult {
        path: output_path.to_string_lossy().into_owned(),
        width: result.width,
        height: result.height,
        frame_count: result.frame_count,
        duration_ms: result.duration_ms,
        byte_length,
    })
}

fn convert_background(
    input_path: &Path,
    options: &AnimatedBackgroundOptions,
) -> Result<ConversionOutput, BackgroundConversionError> {
    let metadata = fs::metadata(input_path).map_err(|error| {
        io_error("The animated background could not be read", error)
    })?;
    if !options.allow_oversized_source.unwrap_or(false)
        && metadata.len() > MAX_INPUT_BYTES
    {
        return Err(BackgroundConversionError::new(
            "too_large",
            "The animated background source is too large",
        ));
    }

    let bytes = fs::read(input_path).map_err(|error| {
        io_error("The animated background could not be read", error)
    })?;
    let format = detect_format(&bytes)?;

    let backend = options.backend.unwrap_or(ConversionBackend::Auto);
    ensure_backend(backend, format, options.ffmpeg_path.is_some())?;

    let preserve_original_quality =
        options.preserve_original_quality.unwrap_or(false);
    let (output_width, output_height) = clamp_output_dimensions(
        options.output_width,
        options.output_height,
        if preserve_original_quality {
            MAX_PRESERVE_DIMENSION
        } else {
            MAX_OUTPUT_DIMENSION
        },
    );
    let quality = options
        .quality
        .unwrap_or(if preserve_original_quality {
            PRESERVE_QUALITY
        } else {
            DEFAULT_QUALITY
        })
        .clamp(0.0, 100.0);
    let max_frames = options
        .max_frames
        .unwrap_or(if preserve_original_quality {
            PRESERVE_MAX_FRAMES
        } else {
            MAX_FRAMES
        })
        .clamp(1, PRESERVE_MAX_FRAMES);
    let max_duration_ms = options
        .max_duration_ms
        .unwrap_or(if preserve_original_quality {
            PRESERVE_MAX_DURATION_MS
        } else {
            MAX_DURATION_MS
        })
        .clamp(1, PRESERVE_MAX_DURATION_MS);
    let max_output_bytes = options
        .max_output_bytes
        .unwrap_or(if preserve_original_quality {
            MAX_PRESERVED_OUTPUT_BYTES
        } else {
            MAX_OUTPUT_BYTES
        })
        .clamp(1, MAX_PRESERVED_OUTPUT_BYTES);
    let min_frame_interval_ms = if preserve_original_quality {
        1
    } else {
        MIN_FRAME_INTERVAL_MS
    };

    let limits = decode_limits();
    match format {
        AnimatedFormat::Gif => {
            let mut decoder = GifDecoder::new(Cursor::new(bytes.as_slice()))
                .map_err(|error| {
                    decode_error("The GIF could not be decoded", error)
                })?;
            decoder.set_limits(limits).map_err(|error| {
                decode_error("The GIF could not be decoded", error)
            })?;
            encode_frames(
                decoder,
                options.crop,
                output_width,
                output_height,
                quality,
                max_frames,
                max_duration_ms,
                max_output_bytes,
                min_frame_interval_ms,
            )
        }
        AnimatedFormat::Apng => {
            let mut decoder = PngDecoder::new(Cursor::new(bytes.as_slice()))
                .map_err(|error| {
                    decode_error("The APNG could not be decoded", error)
                })?;
            decoder.set_limits(limits).map_err(|error| {
                decode_error("The APNG could not be decoded", error)
            })?;
            let apng = decoder.apng().map_err(|error| {
                decode_error("The APNG could not be decoded", error)
            })?;
            encode_frames(
                apng,
                options.crop,
                output_width,
                output_height,
                quality,
                max_frames,
                max_duration_ms,
                max_output_bytes,
                min_frame_interval_ms,
            )
        }
        AnimatedFormat::AnimatedWebp => {
            let mut decoder = WebPDecoder::new(Cursor::new(bytes.as_slice()))
                .map_err(|error| {
                decode_error("The WebP could not be decoded", error)
            })?;
            decoder.set_limits(limits).map_err(|error| {
                decode_error("The WebP could not be decoded", error)
            })?;
            encode_frames(
                decoder,
                options.crop,
                output_width,
                output_height,
                quality,
                max_frames,
                max_duration_ms,
                max_output_bytes,
                min_frame_interval_ms,
            )
        }
        AnimatedFormat::Avif | AnimatedFormat::Mp4 => {
            Err(BackgroundConversionError::new(
                "backend_unavailable",
                "This format needs the FFmpeg conversion backend",
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_frames<'a, D>(
    decoder: D,
    crop: NormalizedCropRect,
    output_width: u32,
    output_height: u32,
    quality: f32,
    max_frames: u32,
    max_duration_ms: u64,
    max_output_bytes: u64,
    min_frame_interval_ms: u64,
) -> Result<ConversionOutput, BackgroundConversionError>
where
    D: AnimationDecoder<'a>,
{
    let options = EncoderOptions {
        anim_params: AnimParams { loop_count: 0 },
        encoding_config: Some(EncodingConfig::new_lossy(quality)),
        ..Default::default()
    };
    let mut encoder =
        Encoder::new_with_options((output_width, output_height), options)
            .map_err(|error| {
                encode_error("The WebP encoder could not start", error)
            })?;

    let mut elapsed_ms = 0u64;
    let mut last_encoded_at: Option<u64> = None;
    let mut frame_count = 0u32;
    let mut last_frame_duration = 0u64;

    for frame in decoder.into_frames() {
        let frame = frame.map_err(|error| {
            decode_error("An animation frame could not be decoded", error)
        })?;
        let delay_ms = frame_delay_ms(&frame);
        let buffer = frame.into_buffer();
        if buffer.width() == 0 || buffer.height() == 0 {
            continue;
        }

        let should_encode = match last_encoded_at {
            None => true,
            Some(last) => {
                elapsed_ms.saturating_sub(last) >= min_frame_interval_ms
            }
        };

        if should_encode {
            let fitted =
                crop_and_resize(&buffer, crop, output_width, output_height)?;
            let timestamp = i32::try_from(elapsed_ms).map_err(|_| {
                BackgroundConversionError::new(
                    "too_long",
                    "The animation is too long",
                )
            })?;
            encoder
                .add_frame(fitted.as_raw(), timestamp)
                .map_err(|error| {
                    encode_error("A WebP frame could not be encoded", error)
                })?;
            last_encoded_at = Some(elapsed_ms);
            last_frame_duration = delay_ms.max(1);
            frame_count += 1;
        }

        elapsed_ms = elapsed_ms.saturating_add(delay_ms.max(1));
        if frame_count >= max_frames || elapsed_ms >= max_duration_ms {
            break;
        }
    }

    if frame_count == 0 {
        return Err(BackgroundConversionError::new(
            "decode_failed",
            "The animation did not contain any usable frames",
        ));
    }

    let last_start = last_encoded_at.unwrap_or(0);
    let end_timestamp = elapsed_ms
        .max(last_start.saturating_add(last_frame_duration.max(1)))
        .min(max_duration_ms.max(1));
    let end_timestamp = i32::try_from(end_timestamp).unwrap_or(i32::MAX).max(
        i32::try_from(last_start)
            .unwrap_or(i32::MAX)
            .saturating_add(1),
    );

    let data = encoder.finalize(end_timestamp).map_err(|error| {
        encode_error("The animated WebP could not be assembled", error)
    })?;
    let bytes = data.as_ref().to_vec();
    if bytes.len() as u64 > max_output_bytes {
        return Err(BackgroundConversionError::new(
            "output_too_large",
            "The encoded animated background is too large",
        ));
    }

    Ok(ConversionOutput {
        width: output_width,
        height: output_height,
        frame_count,
        duration_ms: end_timestamp.max(0) as u64,
        bytes,
    })
}

fn crop_and_resize(
    buffer: &RgbaImage,
    crop: NormalizedCropRect,
    output_width: u32,
    output_height: u32,
) -> Result<RgbaImage, BackgroundConversionError> {
    let source_width = buffer.width();
    let source_height = buffer.height();
    let left = (crop.left * f64::from(source_width)).floor() as u32;
    let top = (crop.top * f64::from(source_height)).floor() as u32;
    let available_width = source_width.saturating_sub(left).max(1);
    let available_height = source_height.saturating_sub(top).max(1);
    let width = ((crop.width * f64::from(source_width)).round() as u32)
        .clamp(1, available_width);
    let height = ((crop.height * f64::from(source_height)).round() as u32)
        .clamp(1, available_height);

    if output_width == 0 || output_height == 0 {
        return Err(BackgroundConversionError::new(
            "invalid_crop",
            "The output dimensions are invalid",
        ));
    }

    let cropped =
        imageops::crop_imm(buffer, left, top, width, height).to_image();
    Ok(imageops::resize(
        &cropped,
        output_width,
        output_height,
        FilterType::Triangle,
    ))
}

fn frame_delay_ms(frame: &Frame) -> u64 {
    let (numerator, denominator) = frame.delay().numer_denom_ms();
    if denominator == 0 {
        0
    } else {
        u64::from(numerator) / u64::from(denominator)
    }
}

fn detect_format(
    bytes: &[u8],
) -> Result<AnimatedFormat, BackgroundConversionError> {
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return if png_declares_animation(bytes) {
            Ok(AnimatedFormat::Apng)
        } else {
            Err(static_image_error())
        };
    }

    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok(AnimatedFormat::Gif);
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
    {
        return if webp_declares_animation(bytes) {
            Ok(AnimatedFormat::AnimatedWebp)
        } else {
            Err(static_image_error())
        };
    }

    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let header = &bytes[..bytes.len().min(64)];
        let is_avif = header
            .windows(4)
            .any(|brand| brand == b"avif" || brand == b"avis");
        return Ok(if is_avif {
            AnimatedFormat::Avif
        } else {
            AnimatedFormat::Mp4
        });
    }

    Err(BackgroundConversionError::new(
        "unsupported_format",
        "The animated background format is not supported",
    ))
}

fn static_image_error() -> BackgroundConversionError {
    BackgroundConversionError::new(
        "unsupported_format",
        "Static images use the image crop flow",
    )
}

fn is_valid_crop(crop: &NormalizedCropRect) -> bool {
    [crop.left, crop.top, crop.width, crop.height]
        .into_iter()
        .all(f64::is_finite)
        && crop.left >= 0.0
        && crop.top >= 0.0
        && crop.width > 0.0
        && crop.height > 0.0
        && crop.left + crop.width <= 1.000_001
        && crop.top + crop.height <= 1.000_001
}

fn ensure_backend(
    backend: ConversionBackend,
    format: AnimatedFormat,
    ffmpeg_configured: bool,
) -> Result<(), BackgroundConversionError> {
    match backend {
        ConversionBackend::Native if !format.is_native_supported() => {
            Err(BackgroundConversionError::new(
                "unsupported_format",
                "The native backend does not support this format",
            ))
        }
        ConversionBackend::Auto if !format.is_native_supported() => {
            Err(BackgroundConversionError::new(
                "backend_unavailable",
                "This format needs the FFmpeg conversion backend, which is not configured",
            ))
        }
        ConversionBackend::Ffmpeg => Err(BackgroundConversionError::new(
            "backend_unavailable",
            if ffmpeg_configured {
                "The FFmpeg conversion backend is not available yet"
            } else {
                "This format needs the FFmpeg conversion backend, which is not configured"
            },
        )),
        _ => Ok(()),
    }
}

fn clamp_output_dimensions(
    width: u32,
    height: u32,
    max_dimension: u32,
) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let longest = width.max(height);
    if longest <= max_dimension {
        return (width, height);
    }

    let scale = f64::from(max_dimension) / f64::from(longest);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits
}

fn validate_output_path<R: Runtime>(
    app: &AppHandle<R>,
    output_path: &Path,
) -> Result<(), BackgroundConversionError> {
    let background_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| {
            BackgroundConversionError::new(
                "io_failed",
                format!(
                    "The app data directory could not be resolved: {error}"
                ),
            )
        })?
        .join("backgrounds");
    fs::create_dir_all(&background_directory).map_err(|error| {
        io_error("The background directory could not be created", error)
    })?;
    let canonical_directory =
        fs::canonicalize(&background_directory).map_err(|error| {
            io_error("The background directory could not be resolved", error)
        })?;

    let parent = output_path.parent().ok_or_else(|| {
        BackgroundConversionError::new(
            "invalid_output",
            "The output path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        io_error("The output directory could not be created", error)
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        io_error("The output directory could not be resolved", error)
    })?;
    if canonical_parent != canonical_directory {
        return Err(BackgroundConversionError::new(
            "invalid_output",
            "Animated backgrounds must be stored in the backgrounds directory",
        ));
    }

    let file_name = output_path.file_name().and_then(|name| name.to_str());
    let Some(file_name) = file_name else {
        return Err(BackgroundConversionError::new(
            "invalid_output",
            "The output path has no file name",
        ));
    };
    if !file_name.starts_with("launcher-background-")
        || output_path
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("webp")
    {
        return Err(BackgroundConversionError::new(
            "invalid_output",
            "The output file name is not a launcher background",
        ));
    }

    Ok(())
}

fn write_atomically(
    output_path: &Path,
    bytes: &[u8],
) -> Result<(), BackgroundConversionError> {
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            BackgroundConversionError::new(
                "invalid_output",
                "The output path has no file name",
            )
        })?;
    let temporary_path =
        output_path.with_file_name(format!(".{file_name}.tmp"));

    fs::write(&temporary_path, bytes).map_err(|error| {
        io_error("The animated background could not be written", error)
    })?;
    if let Err(error) = fs::rename(&temporary_path, output_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(io_error(
            "The animated background could not be moved into place",
            error,
        ));
    }

    Ok(())
}

fn png_declares_animation(bytes: &[u8]) -> bool {
    if bytes.len() < 8 {
        return false;
    }

    let mut offset = 8usize;
    while offset + 8 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let chunk = &bytes[offset + 4..offset + 8];
        if chunk == b"acTL" {
            return true;
        }
        if chunk == b"IDAT" || chunk == b"IEND" {
            return false;
        }
        let next = offset.saturating_add(12).saturating_add(length);
        if next > bytes.len() || next <= offset {
            return false;
        }
        offset = next;
    }

    false
}

fn webp_declares_animation(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }

    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let chunk = &bytes[offset..offset + 4];
        let length = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        if chunk == b"ANIM" {
            return true;
        }
        let next = offset
            .saturating_add(8)
            .saturating_add(length)
            .saturating_add(length % 2);
        if next > bytes.len() || next <= offset {
            return false;
        }
        offset = next;
    }

    false
}

fn io_error(context: &str, error: std::io::Error) -> BackgroundConversionError {
    BackgroundConversionError::new(
        "write_failed",
        format!("{context}: {error}"),
    )
}

fn decode_error(
    context: &str,
    error: impl fmt::Display,
) -> BackgroundConversionError {
    BackgroundConversionError::new(
        "decode_failed",
        format!("{context}: {error}"),
    )
}

fn encode_error(
    context: &str,
    error: impl fmt::Debug,
) -> BackgroundConversionError {
    BackgroundConversionError::new(
        "encode_failed",
        format!("{context}: {error:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_normalized_crop_rectangles() {
        assert!(is_valid_crop(&NormalizedCropRect {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        }));
        assert!(is_valid_crop(&NormalizedCropRect {
            left: 0.25,
            top: 0.1,
            width: 0.5,
            height: 0.8,
        }));
        assert!(!is_valid_crop(&NormalizedCropRect {
            left: 0.5,
            top: 0.0,
            width: 0.6,
            height: 1.0,
        }));
        assert!(!is_valid_crop(&NormalizedCropRect {
            left: f64::NAN,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        }));
        assert!(!is_valid_crop(&NormalizedCropRect {
            left: 0.0,
            top: 0.0,
            width: 0.0,
            height: 1.0,
        }));
    }

    #[test]
    fn clamps_output_dimensions_without_stretching() {
        assert_eq!(clamp_output_dimensions(1920, 1080, 1920), (1920, 1080));
        assert_eq!(clamp_output_dimensions(3840, 2160, 1920), (1920, 1080));
        assert_eq!(clamp_output_dimensions(400, 300, 1920), (400, 300));
        assert_eq!(clamp_output_dimensions(0, 0, 1920), (1, 1));
    }

    #[test]
    fn detects_animation_chunks_before_pixels() {
        let still_png = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0x49,
            0x44, 0x41, 0x54,
        ];
        let apng = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0, 0x61,
            0x63, 0x54, 0x4c,
        ];
        assert!(!png_declares_animation(&still_png));
        assert!(png_declares_animation(&apng));
    }

    #[test]
    fn detects_animated_webp_chunks() {
        let still = b"RIFF\x04\0\0\0WEBPVP8 ";
        let animated = b"RIFF\x04\0\0\0WEBPANIM\0\0\0\0";
        assert!(!webp_declares_animation(still));
        assert!(webp_declares_animation(animated));
    }

    #[test]
    fn detects_containers_by_magic_bytes() {
        assert_eq!(detect_format(b"GIF89a....").unwrap(), AnimatedFormat::Gif);
        assert_eq!(
            detect_format(&[
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0,
                0x61, 0x63, 0x54, 0x4c,
            ])
            .unwrap(),
            AnimatedFormat::Apng
        );
        assert_eq!(
            detect_format(&[
                0, 0, 0, 0x18, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f',
            ])
            .unwrap(),
            AnimatedFormat::Avif
        );
        assert_eq!(
            detect_format(&[
                0, 0, 0, 0x18, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm',
            ])
            .unwrap(),
            AnimatedFormat::Mp4
        );
    }

    #[test]
    fn maps_frame_delays_to_milliseconds() {
        let frame = Frame::from_parts(
            RgbaImage::new(1, 1),
            0,
            0,
            image::Delay::from_numer_denom_ms(120, 1),
        );
        assert_eq!(frame_delay_ms(&frame), 120);
    }

    #[test]
    fn crops_and_resizes_a_frame() {
        let buffer =
            RgbaImage::from_pixel(100, 50, image::Rgba([10, 20, 30, 255]));
        let cropped = crop_and_resize(
            &buffer,
            NormalizedCropRect {
                left: 0.25,
                top: 0.0,
                width: 0.5,
                height: 1.0,
            },
            40,
            20,
        )
        .expect("crop should succeed");
        assert_eq!(cropped.dimensions(), (40, 20));
    }

    #[test]
    fn rejects_formats_outside_the_native_backend() {
        assert!(
            ensure_backend(ConversionBackend::Auto, AnimatedFormat::Gif, false)
                .is_ok()
        );
        assert_eq!(
            ensure_backend(
                ConversionBackend::Native,
                AnimatedFormat::Mp4,
                false
            )
            .expect_err("native MP4 should fail")
            .code,
            "unsupported_format"
        );
        assert_eq!(
            ensure_backend(
                ConversionBackend::Auto,
                AnimatedFormat::Avif,
                false
            )
            .expect_err("AVIF needs FFmpeg")
            .code,
            "backend_unavailable"
        );
        assert_eq!(
            ensure_backend(
                ConversionBackend::Ffmpeg,
                AnimatedFormat::Gif,
                true
            )
            .expect_err("FFmpeg backend is not implemented")
            .code,
            "backend_unavailable"
        );
    }

    #[test]
    fn converts_animated_webp_with_crop() {
        let options = EncoderOptions {
            encoding_config: Some(EncodingConfig::new_lossy(80.0)),
            ..Default::default()
        };
        let mut encoder = Encoder::new_with_options((64, 32), options).unwrap();
        for index in 0u8..3 {
            let data = [index * 40, 0, 0, 255].repeat(64 * 32);
            encoder.add_frame(&data, i32::from(index) * 100).unwrap();
        }
        let input = encoder.finalize(300).unwrap().as_ref().to_vec();

        assert_eq!(
            detect_format(&input).unwrap(),
            AnimatedFormat::AnimatedWebp
        );

        let decoder = WebPDecoder::new(Cursor::new(input.as_slice())).unwrap();
        let output = encode_frames(
            decoder,
            NormalizedCropRect {
                left: 0.0,
                top: 0.0,
                width: 1.0,
                height: 1.0,
            },
            32,
            16,
            75.0,
            10,
            5_000,
            1_000_000,
            50,
        )
        .expect("animated WebP should convert");

        assert_eq!(output.frame_count, 3);
        assert_eq!((output.width, output.height), (32, 16));
        assert!(output.bytes.starts_with(b"RIFF"));

        let mut output_decoder =
            WebPDecoder::new(Cursor::new(output.bytes.as_slice())).unwrap();
        let frames = output_decoder
            .into_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().dimensions(), (32, 16));
    }

    #[test]
    fn converts_apng_with_crop() {
        let mut input = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut input, 32, 16);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.set_frame_delay(100, 1000).unwrap();
            writer
                .write_image_data(&[255, 0, 0, 255].repeat(32 * 16))
                .unwrap();
            writer.set_frame_delay(100, 1000).unwrap();
            writer
                .write_image_data(&[0, 0, 255, 255].repeat(32 * 16))
                .unwrap();
            writer.finish().unwrap();
        }

        assert_eq!(detect_format(&input).unwrap(), AnimatedFormat::Apng);

        let mut decoder =
            PngDecoder::new(Cursor::new(input.as_slice())).unwrap();
        decoder.set_limits(decode_limits()).unwrap();
        let apng = decoder.apng().unwrap();
        let output = encode_frames(
            apng,
            NormalizedCropRect {
                left: 0.0,
                top: 0.0,
                width: 1.0,
                height: 1.0,
            },
            16,
            8,
            75.0,
            10,
            5_000,
            1_000_000,
            50,
        )
        .expect("APNG should convert");

        assert_eq!(output.frame_count, 2);
        assert_eq!((output.width, output.height), (16, 8));

        let mut output_decoder =
            WebPDecoder::new(Cursor::new(output.bytes.as_slice())).unwrap();
        let frames = output_decoder
            .into_frames()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].buffer().dimensions(), (16, 8));
    }
}
