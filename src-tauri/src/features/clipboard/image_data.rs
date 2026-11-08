use std::io::Cursor;

use image::{
    codecs::png::{CompressionType, FilterType, PngEncoder},
    ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Limits,
};

pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_SIDE: u32 = 16384;
pub const MAX_ENCODED_BYTES: usize = 32 * 1024 * 1024;
pub const THUMBNAIL_SIDE: u32 = 256;
pub const MAX_THUMBNAIL_BYTES: usize = 512 * 1024;

#[derive(Clone)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err("Ảnh clipboard vượt giới hạn 16 megapixel.".into());
    }
    Ok(())
}

pub fn decode_png(bytes: &[u8]) -> Result<ClipboardImage, String> {
    if bytes.len() > MAX_ENCODED_BYTES
        || bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
    {
        return Err("Dữ liệu ảnh PNG không hợp lệ hoặc quá lớn.".into());
    }
    let width = u32::from_be_bytes(
        bytes[16..20]
            .try_into()
            .map_err(|_| "Thiếu kích thước ảnh")?,
    );
    let height = u32::from_be_bytes(
        bytes[20..24]
            .try_into()
            .map_err(|_| "Thiếu kích thước ảnh")?,
    );
    validate_dimensions(width, height)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("Không thể đọc ảnh clipboard: {error}"))?
        .into_rgba8();
    Ok(ClipboardImage {
        width,
        height,
        rgba: image.into_raw(),
    })
}

pub fn encode_png(image: &ClipboardImage) -> Result<Vec<u8>, String> {
    validate_dimensions(image.width, image.height)?;
    let expected = image.width as usize * image.height as usize * 4;
    if image.rgba.len() != expected {
        return Err("Kích thước dữ liệu ảnh không hợp lệ.".into());
    }
    let mut encoded = Vec::new();
    PngEncoder::new_with_quality(&mut encoded, CompressionType::Fast, FilterType::Adaptive)
        .write_image(
            &image.rgba,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("Không thể lưu ảnh clipboard: {error}"))?;
    if encoded.len() > MAX_ENCODED_BYTES {
        return Err("Ảnh PNG vượt giới hạn 32 MB mỗi ảnh.".into());
    }
    Ok(encoded)
}

pub fn encode_thumbnail(image: &ClipboardImage) -> Result<Vec<u8>, String> {
    validate_dimensions(image.width, image.height)?;
    let pixels = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        image.width,
        image.height,
        image.rgba.as_slice(),
    )
    .ok_or("Dữ liệu ảnh thu nhỏ không hợp lệ.")?;
    let divisor = u64::from(image.width.max(image.height).max(THUMBNAIL_SIDE));
    let width = (u64::from(image.width) * u64::from(THUMBNAIL_SIDE) / divisor).max(1) as u32;
    let height = (u64::from(image.height) * u64::from(THUMBNAIL_SIDE) / divisor).max(1) as u32;
    let thumbnail = image::imageops::thumbnail(&pixels, width, height);
    let encoded = encode_png(&ClipboardImage {
        width: thumbnail.width(),
        height: thumbnail.height(),
        rgba: thumbnail.into_raw(),
    })?;
    if encoded.len() > MAX_THUMBNAIL_BYTES {
        return Err("Ảnh xem trước vượt giới hạn an toàn.".into());
    }
    Ok(encoded)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("Dữ liệu bitmap bị thiếu.")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("Dữ liệu bitmap bị thiếu.")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

pub fn decode_dib(bytes: &[u8]) -> Result<ClipboardImage, String> {
    let header_size = u32_at(bytes, 0)? as usize;
    if header_size < 40 || header_size > bytes.len() {
        return Err("Định dạng bitmap clipboard chưa được hỗ trợ.".into());
    }
    let width_signed = u32_at(bytes, 4)? as i32;
    let height_signed = u32_at(bytes, 8)? as i32;
    if width_signed <= 0
        || height_signed == 0
        || height_signed == i32::MIN
        || u16_at(bytes, 12)? != 1
    {
        return Err("Kích thước bitmap clipboard không hợp lệ.".into());
    }
    let width = width_signed as u32;
    let height = height_signed.unsigned_abs();
    validate_dimensions(width, height)?;
    let bits = u16_at(bytes, 14)?;
    let compression = u32_at(bytes, 16)?;
    if ![24, 32].contains(&bits)
        || ![0, 3].contains(&compression)
        || (bits == 24 && compression != 0)
    {
        return Err("Chỉ hỗ trợ bitmap RGB 24 hoặc 32 bit trong clipboard.".into());
    }
    let mut offset = header_size;
    let mut explicit_alpha = false;
    if compression == 3 {
        if header_size == 40 {
            offset = offset.checked_add(12).ok_or("Bitmap quá lớn.")?;
        }
        let masks = [u32_at(bytes, 40)?, u32_at(bytes, 44)?, u32_at(bytes, 48)?];
        if masks != [0x00ff0000, 0x0000ff00, 0x000000ff] {
            return Err("Định dạng màu bitmap clipboard chưa được hỗ trợ.".into());
        }
        if header_size >= 56 {
            let alpha_mask = u32_at(bytes, 52)?;
            if alpha_mask != 0 && alpha_mask != 0xff000000 {
                return Err("Kênh trong suốt bitmap không hợp lệ.".into());
            }
            explicit_alpha = alpha_mask == 0xff000000;
        }
    }
    if header_size >= 124 && u32_at(bytes, 116)? != 0 {
        return Err(
            "Ảnh bitmap có hồ sơ màu nhúng chưa được hỗ trợ; hãy sao chép dưới dạng PNG.".into(),
        );
    }
    // True-color DIBs may still carry an optional optimization palette. Its
    // RGBQUAD entries precede the pixel array, even though pixels are not indices.
    let palette_bytes = (u32_at(bytes, 32)? as usize)
        .checked_mul(4)
        .ok_or("Bảng màu bitmap quá lớn.")?;
    offset = offset
        .checked_add(palette_bytes)
        .ok_or("Bảng màu bitmap quá lớn.")?;
    let stride = (width as usize * bits as usize).div_ceil(32) * 4;
    let length = stride
        .checked_mul(height as usize)
        .and_then(|size| offset.checked_add(size))
        .ok_or("Bitmap quá lớn.")?;
    if length > bytes.len() {
        return Err("Dữ liệu bitmap clipboard bị cắt ngắn.".into());
    }
    // Windows' CF_DIB-to-CF_DIBV5 conversion can copy the three legacy
    // RGB masks after the V5 header even though they are already inside it.
    // Require both the exact surplus and the duplicate masks, so standard
    // packed DIBs that start with red, green and blue pixels stay intact.
    let rgb_mask_bytes = 3 * std::mem::size_of::<u32>();
    if header_size == 124
        && compression == 3
        && palette_bytes == 0
        && bytes.len() - length == rgb_mask_bytes
        && bytes[offset..offset + rgb_mask_bytes] == bytes[40..40 + rgb_mask_bytes]
    {
        offset += rgb_mask_bytes;
    }
    let mut rgba = vec![0; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let source_y = if height_signed < 0 {
            y
        } else {
            height as usize - 1 - y
        };
        for x in 0..width as usize {
            let source = offset + source_y * stride + x * (bits as usize / 8);
            let target = (y * width as usize + x) * 4;
            rgba[target..target + 3].copy_from_slice(&[
                bytes[source + 2],
                bytes[source + 1],
                bytes[source],
            ]);
            // BI_RGB's high byte is reserved, not alpha. Only a declared alpha
            // mask may make pixels transparent (BITMAPV5HEADER documentation).
            rgba[target + 3] = if explicit_alpha {
                bytes[source + 3]
            } else {
                255
            };
        }
    }
    Ok(ClipboardImage {
        width,
        height,
        rgba,
    })
}

pub fn encode_dib_v5(image: &ClipboardImage) -> Result<Vec<u8>, String> {
    validate_dimensions(image.width, image.height)?;
    if image.rgba.len() != image.width as usize * image.height as usize * 4 {
        return Err("Dữ liệu ảnh không hợp lệ.".into());
    }
    let mut dib = vec![0u8; 124 + image.rgba.len()];
    let write = |buffer: &mut [u8], offset, value: u32| {
        buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
    };
    write(&mut dib, 0, 124);
    write(&mut dib, 4, image.width);
    write(&mut dib, 8, (-(image.height as i32)) as u32);
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32u16.to_le_bytes());
    write(&mut dib, 16, 3);
    write(&mut dib, 20, image.rgba.len() as u32);
    for (offset, value) in [
        (40, 0x00ff0000),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
        (56, 0x73524742),
        (108, 4),
    ] {
        write(&mut dib, offset, value);
    }
    for (source, target) in image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dib[124..].as_chunks_mut::<4>().0.iter_mut())
    {
        target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
    }
    Ok(dib)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_dib_roundtrip_preserves_rows_colors_and_transparency() {
        let original = ClipboardImage {
            width: 2,
            height: 2,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 1, 2, 3, 255],
        };
        let encoded = encode_dib_v5(&original).unwrap();
        let decoded = decode_dib(&encoded).unwrap();
        assert_eq!(decoded.rgba, original.rgba);
    }

    #[test]
    fn dib_v5_skips_duplicated_rgb_masks_only_with_the_extra_bytes() {
        let original = ClipboardImage {
            width: 4,
            height: 1,
            rgba: vec![255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 15, 40, 90, 128],
        };
        let packed = encode_dib_v5(&original).unwrap();
        assert_eq!(&packed[124..136], &packed[40..52]);
        assert_eq!(decode_dib(&packed).unwrap().rgba, original.rgba);

        let mut duplicated = packed[..124].to_vec();
        duplicated.extend_from_slice(&packed[40..52]);
        duplicated.extend_from_slice(&packed[124..]);
        assert_eq!(decode_dib(&duplicated).unwrap().rgba, original.rgba);
    }

    fn rgb_dib(width: u32, height: i32, bits: u16, palette: &[[u8; 4]], pixels: &[u8]) -> Vec<u8> {
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&width.to_le_bytes());
        dib[8..12].copy_from_slice(&height.to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&bits.to_le_bytes());
        dib[32..36].copy_from_slice(&(palette.len() as u32).to_le_bytes());
        for color in palette {
            dib.extend_from_slice(color);
        }
        dib.extend_from_slice(pixels);
        dib
    }

    #[test]
    fn rgb32_reserved_bytes_cannot_make_opaque_pixels_transparent() {
        let dib = rgb_dib(2, -1, 32, &[], &[3, 2, 1, 0, 6, 5, 4, 128]);
        assert_eq!(decode_dib(&dib).unwrap().rgba, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn rgb24_palette_padding_and_bottom_up_rows_are_respected() {
        // Each 1px row occupies 4 bytes. The source stores bottom blue then top red.
        let dib = rgb_dib(
            1,
            2,
            24,
            &[[90, 80, 70, 0]],
            &[255, 0, 0, 99, 0, 0, 255, 88],
        );
        assert_eq!(
            decode_dib(&dib).unwrap().rgba,
            [255, 0, 0, 255, 0, 0, 255, 255]
        );
        let mut malformed = dib;
        malformed[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_dib(&malformed).is_err());
    }

    #[test]
    fn corrupt_bitmap_and_decompression_bomb_dimensions_are_rejected() {
        assert!(decode_dib(&[0; 12]).is_err());
        assert!(validate_dimensions(16384, 16384).is_err());
        let original = ClipboardImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        };
        let mut dib = encode_dib_v5(&original).unwrap();
        dib.truncate(125);
        assert!(decode_dib(&dib).is_err());
    }

    #[test]
    fn png_roundtrip_preserves_pixels() {
        let original = ClipboardImage {
            width: 1,
            height: 1,
            rgba: vec![50, 100, 150, 200],
        };
        assert_eq!(
            decode_png(&encode_png(&original).unwrap()).unwrap().rgba,
            original.rgba
        );
    }

    #[test]
    fn preview_is_bounded_and_preserves_aspect_ratio() {
        let original = ClipboardImage {
            width: 1024,
            height: 512,
            rgba: vec![100; 1024 * 512 * 4],
        };
        let preview = decode_png(&encode_thumbnail(&original).unwrap()).unwrap();
        assert_eq!((preview.width, preview.height), (256, 128));
        let small = ClipboardImage {
            width: 2,
            height: 1,
            rgba: vec![255; 8],
        };
        let preview = decode_png(&encode_thumbnail(&small).unwrap()).unwrap();
        assert_eq!((preview.width, preview.height), (2, 1));
    }
}
