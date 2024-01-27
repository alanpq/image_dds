#![no_main]

extern crate libfuzzer_sys;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: [u8; 8]| {
    // 4x4 RGBA u8
    let mut expected = [255u8; 4 * 4 * 4];
    unsafe {
        bcndecode_sys::bcdec_bc1(data.as_ptr(), expected.as_mut_ptr() as _, 4 * 4);
    }

    let mut actual = [255u8; 4 * 4 * 4];
    bcdec_rs::bc1(&data, &mut actual, 4 * 4);

    assert_eq!(expected, actual);
});
