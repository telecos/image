#![cfg(all(feature = "jbig2", feature = "bmp"))]

use std::env;
use std::path::PathBuf;

use image::ColorType;

fn compare_official_pixels(case: &str) {
    let root = PathBuf::from(
        env::var_os("JBIG2_ITU_VECTOR_ROOT").expect("set JBIG2_ITU_VECTOR_ROOT; see docs/jbig2.md"),
    );
    let decoded = image::open(root.join(format!("{case}.jb2"))).unwrap();
    let reference = image::open(root.join(format!("{case}_TT00.bmp")))
        .unwrap()
        .into_rgb8();
    assert_eq!((decoded.width(), decoded.height()), reference.dimensions());
    assert_eq!(decoded.color(), ColorType::L8);
    assert_eq!(decoded.as_bytes().len(), reference.pixels().len());
    for (index, (&sample, expected)) in decoded
        .as_bytes()
        .iter()
        .zip(reference.pixels())
        .enumerate()
    {
        assert_eq!([sample; 3], expected.0, "{case}: pixel {index}");
    }
}

#[test]
#[ignore = "requires external official ITU assets; see docs/jbig2.md"]
fn itu_f01_200_tt9_matches_official_bmp() {
    compare_official_pixels("F01_200_TT9");
}

#[test]
#[ignore = "requires external official ITU assets; see docs/jbig2.md"]
fn itu_f01_200_tt10_matches_official_bmp() {
    compare_official_pixels("F01_200_TT10");
}
