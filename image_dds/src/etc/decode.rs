pub trait EtcDecode<Pixel> {
    // The decoded 4x4 pixel blocks are in row-major ordering.
    // Fixing the length should reduce the amount of bounds checking.
    fn decompress_block(block: &[u8; 8]) -> [[Pixel; BLOCK_WIDTH]; BLOCK_HEIGHT];
}

/// Decompress the bytes in `data` to the uncompressed RGBA8 format.
pub fn decode_etc<F, T>(width: u32, height: u32, data: &[u8]) -> Result<Vec<T>, SurfaceError>
where
    T: Copy + Default + Pod,
    F: EtcDecode<[T; 4]>,
{
    todo!()
}

impl EtcDecode<[u32; 16]> {}
