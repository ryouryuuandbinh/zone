use std::io::{self, Write};

use image::{
    codecs::png::{CompressionType, FilterType, PngEncoder},
    ExtendedColorType, ImageEncoder, RgbImage,
};
use serde::Deserialize;

#[cfg(windows)]
mod windows;

const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_SIDE: u32 = 16384;
const MAX_PNG_BYTES: usize = 32 * 1024 * 1024;

/// Physical monitor pixels in the Windows virtual desktop coordinate space.
#[derive(Clone, Copy)]
pub struct ScreenBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A rectangle in the original snapshot, measured from its top-left pixel.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PixelRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct CapturedFrame {
    rgb: RgbImage,
}

impl CapturedFrame {
    fn from_rgb(width: u32, height: u32, bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() != pixel_count(width, height)? * 3 {
            return Err("Dữ liệu ảnh chụp không đúng kích thước.".into());
        }
        let rgb = RgbImage::from_raw(width, height, bytes)
            .ok_or("Dữ liệu ảnh chụp không đúng kích thước.")?;
        Ok(Self { rgb })
    }

    pub fn width(&self) -> u32 {
        self.rgb.width()
    }

    pub fn height(&self) -> u32 {
        self.rgb.height()
    }

    pub fn validate_region(&self, region: Option<PixelRegion>) -> Result<(), String> {
        let Some(region) = region else {
            return Ok(());
        };
        if region.width == 0 || region.height == 0 {
            return Err("Vùng chọn phải có ít nhất một điểm ảnh.".into());
        }
        let right = region
            .x
            .checked_add(region.width)
            .filter(|&edge| edge <= self.width());
        let bottom = region
            .y
            .checked_add(region.height)
            .filter(|&edge| edge <= self.height());
        if right.is_none() || bottom.is_none() {
            return Err("Vùng chọn nằm ngoài ảnh chụp.".into());
        }
        Ok(())
    }

    pub fn encode_png(&self, region: Option<PixelRegion>) -> Result<Vec<u8>, String> {
        self.validate_region(region)?;
        let Some(region) = region else {
            return encode_rgb_png(self.rgb.as_raw(), self.width(), self.height());
        };
        if region.width == self.width() && region.height == self.height() {
            return encode_rgb_png(self.rgb.as_raw(), self.width(), self.height());
        }

        let mut cropped = Vec::new();
        let row_len = region.width as usize * 3;
        cropped
            .try_reserve_exact(row_len * region.height as usize)
            .map_err(|_| "Không đủ bộ nhớ để cắt ảnh.")?;
        let stride = self.width() as usize * 3;
        let left = region.x as usize * 3;
        let bytes = self.rgb.as_raw();
        for y in region.y..region.y + region.height {
            let start = y as usize * stride + left;
            cropped.extend_from_slice(&bytes[start..start + row_len]);
        }
        encode_rgb_png(&cropped, region.width, region.height)
    }
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0 || pixels > MAX_PIXELS || width > MAX_SIDE || height > MAX_SIDE {
        return Err("Kích thước màn hình vượt giới hạn chụp 16 megapixel.".into());
    }
    Ok(pixels as usize)
}

#[cfg(windows)]
pub fn capture(bounds: ScreenBounds) -> Result<CapturedFrame, String> {
    windows::capture(bounds)
}

#[cfg(not(windows))]
pub fn capture(_bounds: ScreenBounds) -> Result<CapturedFrame, String> {
    Err("Chụp màn hình hiện chỉ hỗ trợ Windows.".into())
}

#[derive(Default)]
struct PngBytes(Vec<u8>);

impl Write for PngBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let len = self.0.len();
        let end = len
            .checked_add(bytes.len())
            .filter(|&size| size <= MAX_PNG_BYTES)
            .ok_or_else(|| io::Error::other("Ảnh PNG vượt giới hạn 32 MiB."))?;
        let capacity = self.0.capacity();
        if end > capacity {
            let next = end.max(capacity.saturating_mul(2)).min(MAX_PNG_BYTES);
            self.0
                .try_reserve_exact(next - len)
                .map_err(|_| io::Error::other("Không đủ bộ nhớ để tạo ảnh PNG."))?;
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_rgb_png(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut png = PngBytes::default();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive)
        .write_image(rgb, width, height, ExtendedColorType::Rgb8)
        .map_err(|error| format!("Không thể tạo ảnh PNG: {error}"))?;
    Ok(png.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_keeps_row_order_and_rejects_empty_or_outside_regions() -> Result<(), String> {
        let rgb = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
        ];
        let frame = CapturedFrame::from_rgb(3, 2, rgb)?;
        let region = PixelRegion {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        };
        let png = frame.encode_png(Some(region))?;
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|error| error.to_string())?
            .into_rgb8();
        assert_eq!(decoded.dimensions(), (2, 2));
        assert_eq!(
            decoded.as_raw(),
            &[4, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17, 18]
        );
        for invalid in [
            PixelRegion { width: 0, ..region },
            PixelRegion { x: 2, ..region },
            PixelRegion {
                x: u32::MAX,
                ..region
            },
        ] {
            assert!(frame.encode_png(Some(invalid)).is_err());
        }
        Ok(())
    }
}
