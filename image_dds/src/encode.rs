use crate::bcn::{self, Bc1, Bc2, Bc3, Bc4, Bc5, Bc6, Bc7};
use crate::rgba::{bgra8_from_rgba8, encode_rgba8_from_rgba8, r8_from_rgba8, rgbaf32_from_rgba8};
use crate::{
    downsample_rgba8, error::CompressSurfaceError, max_mipmap_count, mip_dimension, round_up,
    ImageFormat, Mipmaps, Quality, Surface, SurfaceRgba8,
};

// TODO: Add documentation showing how to use this.
/// Encode an RGBA8 surface to the given `format`.
///
/// The number of mipmaps generated depends on the `mipmaps` parameter.
/// The `rgba8_data` only needs to contain enough data for the base mip level of `width` x `height` pixels.
pub fn encode_surface_rgba8<T: AsRef<[u8]>>(
    surface: SurfaceRgba8<T>,
    format: ImageFormat,
    quality: Quality,
    mipmaps: Mipmaps,
) -> Result<Surface<Vec<u8>>, CompressSurfaceError> {
    let width = surface.width;
    let height = surface.height;
    let depth = surface.depth;
    let layers = surface.layers;

    surface.validate_encode(format)?;

    // TODO: Encode the correct number of array layers.
    let num_mipmaps = match mipmaps {
        Mipmaps::Disabled => 1,
        Mipmaps::FromSurface => surface.mipmaps,
        Mipmaps::GeneratedExact(count) => count,
        Mipmaps::GeneratedAutomatic => max_mipmap_count(width.max(height).max(depth)),
    };

    let use_surface = mipmaps == Mipmaps::FromSurface;

    // TODO: Does this work if the base mip level is smaller than 4x4?
    let mut surface_data = Vec::new();

    for layer in 0..layers {
        encode_mipmaps_rgba8(
            &mut surface_data,
            &surface,
            format,
            quality,
            num_mipmaps,
            use_surface,
            layer,
        )?;
    }

    Ok(Surface {
        width,
        height,
        depth,
        layers,
        mipmaps: num_mipmaps,
        image_format: format,
        data: surface_data,
    })
}

fn encode_mipmaps_rgba8<T: AsRef<[u8]>>(
    encoded_data: &mut Vec<u8>,
    surface: &SurfaceRgba8<T>,
    format: ImageFormat,
    quality: Quality,
    num_mipmaps: u32,
    use_surface: bool,
    layer: u32,
) -> Result<(), CompressSurfaceError> {
    let width = surface.width;
    let height = surface.height;
    let depth = surface.depth;
    let (block_width, block_height, block_depth) = format.block_dimensions();

    // The base mip level is always included.
    let base_layer = encode_rgba8(
        width.max(block_width),
        height.max(block_height),
        depth.max(block_depth),
        surface.data.as_ref(),
        format,
        quality,
    )?;
    encoded_data.extend_from_slice(&base_layer);

    let mut mip_image = surface.data.as_ref().to_vec();

    let mut previous_width = width as usize;
    let mut previous_height = height as usize;
    let mut previous_depth = depth as usize;

    for mipmap in 1..num_mipmaps {
        // The physical size must have integral dimensions in blocks.
        // Applications or the GPU will use the smaller virtual size and ignore padding.
        // For example, a 1x1 BCN block still requires 4x4 pixels of data.
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression
        let mip_dimension_rounded = |x, n| round_up(mip_dimension(x, mipmap) as usize, n as usize);
        let mip_width = mip_dimension_rounded(width, block_width);
        let mip_height = mip_dimension_rounded(height, block_height);
        let mip_depth = mip_dimension_rounded(depth, block_depth);

        // TODO: Find a simpler way to choose a data source.
        mip_image = if use_surface {
            // TODO: Array layers.
            // TODO: Avoid unwrap
            let data = surface.get(layer, mipmap).unwrap();
            let expected_size = mip_width * mip_height * mip_depth * 4;

            if data.len() < expected_size {
                // Zero pad the data to the appropriate size.
                let mut padded_data = vec![0u8; expected_size];
                for z in 0..mip_depth {
                    for y in 0..mip_height {
                        for x in 0..mip_width {
                            // TODO: Make this copy technique a helper function?
                            // TODO: Optimize this for known pixel sizes?
                            // This can't be a memory copy because of the stride.
                            let i = (z * mip_width * mip_height) + y * mip_width + x;
                            padded_data[i] = data[i];
                        }
                    }
                }

                padded_data
            } else {
                data.to_vec()
            }
        } else {
            // Downsample the previous mip level.
            // This also handles padding since the new dimensions are rounded.
            downsample_rgba8(
                mip_width,
                mip_height,
                mip_depth,
                previous_width,
                previous_height,
                previous_depth,
                &mip_image,
            )
        };

        let mip_data = encode_rgba8(
            mip_width as u32,
            mip_height as u32,
            mip_depth as u32,
            &mip_image,
            format,
            quality,
        )?;
        encoded_data.extend_from_slice(&mip_data);

        // Update the dimensions for the previous mipmap image data.
        previous_width = mip_width;
        previous_height = mip_height;
        previous_depth = mip_depth;
    }
    Ok(())
}

fn encode_rgba8(
    width: u32,
    height: u32,
    depth: u32,
    data: &[u8],
    format: ImageFormat,
    quality: Quality,
) -> Result<Vec<u8>, CompressSurfaceError> {
    // TODO: Handle unorm vs srgb for uncompressed or leave the data as is?

    use ImageFormat as F;
    match format {
        F::BC1Unorm | F::BC1Srgb => bcn::bcn_from_rgba8::<Bc1>(width, height, depth, data, quality),
        F::BC2Unorm | F::BC2Srgb => bcn::bcn_from_rgba8::<Bc2>(width, height, depth, data, quality),
        F::BC3Unorm | F::BC3Srgb => bcn::bcn_from_rgba8::<Bc3>(width, height, depth, data, quality),
        F::BC4Unorm | F::BC4Snorm => {
            bcn::bcn_from_rgba8::<Bc4>(width, height, depth, data, quality)
        }
        F::BC5Unorm | F::BC5Snorm => {
            bcn::bcn_from_rgba8::<Bc5>(width, height, depth, data, quality)
        }
        F::BC6Ufloat | F::BC6Sfloat => {
            bcn::bcn_from_rgba8::<Bc6>(width, height, depth, data, quality)
        }
        F::BC7Unorm | F::BC7Srgb => bcn::bcn_from_rgba8::<Bc7>(width, height, depth, data, quality),
        F::R8Unorm => r8_from_rgba8(width, height, depth, data),
        F::R8G8B8A8Unorm => encode_rgba8_from_rgba8(width, height, depth, data),
        F::R8G8B8A8Srgb => encode_rgba8_from_rgba8(width, height, depth, data),
        F::R32G32B32A32Float => rgbaf32_from_rgba8(width, height, depth, data),
        F::B8G8R8A8Unorm => bgra8_from_rgba8(width, height, depth, data),
        F::B8G8R8A8Srgb => bgra8_from_rgba8(width, height, depth, data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_surface_integral_dimensions() {
        // It's ok for mipmaps to not be divisible by the block width.
        let surface = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 12,
                height: 12,
                depth: 1,
                layers: 1,
                mipmaps: 1,
                data: &[0u8; 12 * 12 * 4],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::GeneratedAutomatic,
        )
        .unwrap();

        assert_eq!(12, surface.width);
        assert_eq!(12, surface.height);
        assert_eq!(1, surface.depth);
        assert_eq!(1, surface.layers);
        assert_eq!(4, surface.mipmaps);
        assert_eq!(ImageFormat::BC7Srgb, surface.image_format);
        // Each mipmap must be at least 1 block in size.
        assert_eq!((9 + 4 + 1 + 1) * 16, surface.data.len());
    }

    #[test]
    fn encode_surface_cube_mipmaps() {
        // It's ok for mipmaps to not be divisible by the block width.
        let surface = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 4,
                height: 4,
                depth: 1,
                layers: 6,
                mipmaps: 3,
                data: &[0u8; (4 * 4 + 2 * 2 + 1 * 1) * 6 * 4],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::GeneratedAutomatic,
        )
        .unwrap();

        assert_eq!(4, surface.width);
        assert_eq!(4, surface.height);
        assert_eq!(1, surface.depth);
        assert_eq!(6, surface.layers);
        assert_eq!(3, surface.mipmaps);
        assert_eq!(ImageFormat::BC7Srgb, surface.image_format);
        // Each mipmap must be at least 1 block in size.
        assert_eq!(3 * 16 * 6, surface.data.len());
    }

    #[test]
    fn encode_surface_disabled_mipmaps() {
        let surface = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 4,
                height: 4,
                depth: 1,
                layers: 1,
                mipmaps: 3,
                data: &[0u8; 64 + 16 + 4],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::Disabled,
        )
        .unwrap();

        assert_eq!(4, surface.width);
        assert_eq!(4, surface.height);
        assert_eq!(1, surface.depth);
        assert_eq!(1, surface.layers);
        assert_eq!(1, surface.mipmaps);
        assert_eq!(ImageFormat::BC7Srgb, surface.image_format);
        assert_eq!(16, surface.data.len());
    }

    #[test]
    fn encode_surface_mipmaps_from_surface() {
        let surface = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 4,
                height: 4,
                depth: 1,
                layers: 1,
                mipmaps: 2,
                data: &[0u8; 64 + 16],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::FromSurface,
        )
        .unwrap();

        assert_eq!(4, surface.width);
        assert_eq!(4, surface.height);
        assert_eq!(1, surface.depth);
        assert_eq!(1, surface.layers);
        assert_eq!(2, surface.mipmaps);
        assert_eq!(ImageFormat::BC7Srgb, surface.image_format);
        assert_eq!(16 * 2, surface.data.len());
    }

    #[test]
    fn encode_surface_non_integral_dimensions() {
        // This should still fail even though there is enough data.
        let result = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 3,
                height: 5,
                depth: 2,
                layers: 1,
                mipmaps: 1,
                data: &[0u8; 256],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::GeneratedAutomatic,
        );
        assert!(matches!(
            result,
            Err(CompressSurfaceError::NonIntegralDimensionsInBlocks {
                width: 3,
                height: 5,
                depth: 2,
                block_width: 4,
                block_height: 4
            })
        ));
    }

    #[test]
    fn encode_surface_zero_size() {
        let result = encode_surface_rgba8(
            SurfaceRgba8 {
                width: 0,
                height: 0,
                depth: 0,
                layers: 1,
                mipmaps: 1,
                data: &[0u8; 0],
            },
            ImageFormat::BC7Srgb,
            Quality::Fast,
            Mipmaps::GeneratedAutomatic,
        );
        assert!(matches!(
            result,
            Err(CompressSurfaceError::ZeroSizedSurface {
                width: 0,
                height: 0,
                depth: 0,
            })
        ));
    }
}
