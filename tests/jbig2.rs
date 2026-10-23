use image::{ImageError, ImageFormat};

const MAGIC: &[u8] = b"\x97JB2\r\n\x1a\n";

#[test]
fn recognizes_jbig2_independently_of_decoder_feature() {
    assert_eq!(image::guess_format(MAGIC).unwrap(), ImageFormat::Jbig2);
    for extension in ["jb2", "JB2", "jbig2"] {
        assert_eq!(
            ImageFormat::from_extension(extension),
            Some(ImageFormat::Jbig2)
        );
    }
    assert_eq!(ImageFormat::Jbig2.extensions_str(), &["jb2", "jbig2"]);
    assert!(ImageFormat::all().any(|format| format == ImageFormat::Jbig2));
    assert_eq!(
        ImageFormat::Jbig2.reading_enabled(),
        cfg!(feature = "jbig2")
    );
    assert!(!ImageFormat::Jbig2.writing_enabled());
    assert!(ImageFormat::from_mime_type("application/octet-stream").is_none());
}

#[cfg(not(feature = "jbig2"))]
#[test]
fn disabled_decoder_returns_unsupported() {
    assert!(matches!(
        image::load_from_memory(MAGIC),
        Err(ImageError::Unsupported(_))
    ));
}

#[cfg(feature = "jbig2")]
mod decode {
    use std::error::Error;
    use std::io::{self, Cursor, Read, Seek, SeekFrom};

    use image::codecs::jbig2::Jbig2Decoder;
    use image::{ColorType, ImageDecoder, ImageReaderOptions, Limits};

    use super::*;

    struct Segment(u8, u8, Vec<u8>);

    fn document(sequential: bool, count: Option<u32>, segments: &[Segment]) -> Vec<u8> {
        let mut result = MAGIC.to_vec();
        result.push(u8::from(sequential) | (u8::from(count.is_none()) << 1));
        if let Some(count) = count {
            result.extend_from_slice(&count.to_be_bytes());
        }
        for (number, Segment(kind, page, payload)) in segments.iter().enumerate() {
            result.extend_from_slice(&u32::try_from(number).unwrap().to_be_bytes());
            result.extend_from_slice(&[*kind, 0, *page]);
            result.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
            if sequential {
                result.extend_from_slice(payload);
            }
        }
        if !sequential {
            for Segment(_, _, payload) in segments {
                result.extend_from_slice(payload);
            }
        }
        result
    }

    fn page(width: u32, height: u32, black: bool) -> Vec<u8> {
        let mut payload = width.to_be_bytes().to_vec();
        payload.extend_from_slice(&height.to_be_bytes());
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&[0x40 | (u8::from(black) << 2), 0, 0]);
        payload
    }

    fn blank(width: u32, height: u32, black: bool) -> Vec<u8> {
        document(
            true,
            Some(1),
            &[
                Segment(48, 1, page(width, height, black)),
                Segment(49, 1, vec![]),
                Segment(51, 0, vec![]),
            ],
        )
    }

    fn mmr(entropy: &[u8]) -> Vec<u8> {
        let mut region = 8_u32.to_be_bytes().to_vec();
        region.extend_from_slice(&1_u32.to_be_bytes());
        region.extend_from_slice(&[0; 8]);
        region.extend_from_slice(&[0, 1]);
        region.extend_from_slice(entropy);
        document(
            true,
            Some(1),
            &[
                Segment(48, 1, page(8, 1, false)),
                Segment(38, 1, region),
                Segment(49, 1, vec![]),
                Segment(51, 0, vec![]),
            ],
        )
    }

    #[test]
    fn expands_polarity_and_odd_width_rows_to_l8() {
        for black in [false, true] {
            let encoded = blank(9, 3, black);
            let decoded = image::load_from_memory(&encoded).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (9, 3));
            assert_eq!(decoded.color(), ColorType::L8);
            assert_eq!(decoded.as_bytes(), &[if black { 0 } else { 255 }; 27]);
        }
    }

    #[test]
    fn mmr_pixels_dispatch_through_image() {
        let encoded = mmr(&[0x36, 0xc0]);
        let decoded = image::load_from_memory_with_format(&encoded, ImageFormat::Jbig2).unwrap();
        assert_eq!(decoded.as_bytes(), &[255, 255, 255, 255, 0, 0, 0, 0]);
        let guessed = ImageReaderOptions::new(Cursor::new(encoded))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(guessed, decoded);
    }

    #[test]
    fn accepts_random_access_and_unknown_page_count() {
        for sequential in [true, false] {
            for count in [Some(1), None] {
                let encoded = document(
                    sequential,
                    count,
                    &[
                        Segment(48, 1, page(9, 2, true)),
                        Segment(49, 1, vec![]),
                        Segment(51, 0, vec![]),
                    ],
                );
                let decoded = image::load_from_memory(&encoded).unwrap();
                assert_eq!(decoded.as_bytes(), &[0; 18]);
            }
        }
    }

    #[test]
    fn accepts_unknown_height_striped_page() {
        let mut payload = page(9, u32::MAX, true);
        payload[17..19].copy_from_slice(&0x8002_u16.to_be_bytes());
        let encoded = document(
            true,
            Some(1),
            &[
                Segment(48, 1, payload),
                Segment(50, 1, 1_u32.to_be_bytes().to_vec()),
                Segment(49, 1, vec![]),
                Segment(51, 0, vec![]),
            ],
        );
        let decoded = image::load_from_memory(&encoded).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (9, 2));
        assert_eq!(decoded.as_bytes(), &[0; 18]);
    }

    #[test]
    fn preparation_is_idempotent_and_honours_current_stream_position() {
        let mut input = b"prefix".to_vec();
        input.extend(blank(9, 2, false));
        let mut reader = Cursor::new(input);
        reader.set_position(6);
        let mut decoder = Jbig2Decoder::new(reader);
        for _ in 0..2 {
            let layout = decoder.prepare_image().unwrap();
            assert_eq!((layout.layout.width, layout.layout.height), (9, 2));
            assert_eq!(layout.layout.color, ColorType::L8);
            assert_eq!(layout.total_bytes(), 18);
        }
        let mut output = [0; 18];
        decoder.read_image(&mut output).unwrap();
        assert_eq!(output, [255; 18]);
    }

    #[test]
    fn limits_cover_input_dimensions_and_expanded_output() {
        let encoded = blank(100, 100, false);
        let mut limits = Limits::no_limits();
        limits.max_alloc = Some(u64::try_from(encoded.len()).unwrap() - 1);
        let mut decoder = Jbig2Decoder::new(Cursor::new(&encoded));
        decoder.set_limits(limits).unwrap();
        assert!(matches!(
            decoder.prepare_image(),
            Err(ImageError::Limits(_))
        ));

        for (width, height) in [(Some(99), None), (None, Some(99))] {
            let mut limits = Limits::no_limits();
            limits.max_image_width = width;
            limits.max_image_height = height;
            let mut decoder = Jbig2Decoder::new(Cursor::new(&encoded));
            decoder.set_limits(limits).unwrap();
            assert!(matches!(
                decoder.prepare_image(),
                Err(ImageError::Limits(_))
            ));
            assert!(matches!(
                decoder.prepare_image(),
                Err(ImageError::Limits(_))
            ));
        }
        let mut limits = Limits::no_limits();
        limits.max_alloc = Some(u64::try_from(encoded.len()).unwrap() + 9_999);
        let mut decoder = Jbig2Decoder::new(Cursor::new(&encoded));
        decoder.set_limits(limits).unwrap();
        assert!(matches!(
            decoder.prepare_image(),
            Err(ImageError::Limits(_))
        ));
    }

    #[test]
    fn changing_limits_after_preparation_is_checked() {
        let encoded = blank(9, 2, true);
        let mut decoder = Jbig2Decoder::new(Cursor::new(&encoded));
        decoder.prepare_image().unwrap();
        let mut limits = Limits::no_limits();
        limits.max_image_width = Some(8);
        assert!(matches!(
            decoder.set_limits(limits),
            Err(ImageError::Limits(_))
        ));
        let mut limits = Limits::no_limits();
        limits.max_alloc = Some(u64::try_from(encoded.len()).unwrap() + 17);
        assert!(matches!(
            decoder.set_limits(limits),
            Err(ImageError::Limits(_))
        ));
        decoder.set_limits(Limits::default()).unwrap();
        let mut output = [255; 18];
        decoder.read_image(&mut output).unwrap();
        assert_eq!(output, [0; 18]);
    }

    #[test]
    fn rejects_multiple_pages_instead_of_discarding_them() {
        let encoded = document(
            true,
            Some(2),
            &[
                Segment(48, 1, page(9, 2, false)),
                Segment(49, 1, vec![]),
                Segment(48, 2, page(9, 2, true)),
                Segment(49, 2, vec![]),
                Segment(51, 0, vec![]),
            ],
        );
        assert!(matches!(
            image::load_from_memory(&encoded),
            Err(ImageError::Unsupported(_))
        ));
    }

    #[test]
    fn malformed_input_and_embedded_streams_are_errors() {
        let encoded = blank(9, 2, false);
        for length in [0, 8, 12, encoded.len() - 1] {
            assert!(
                image::load_from_memory_with_format(&encoded[..length], ImageFormat::Jbig2)
                    .is_err()
            );
        }
        assert!(image::load_from_memory_with_format(&encoded[13..], ImageFormat::Jbig2).is_err());
        let error = image::load_from_memory(&mmr(&[0])).unwrap_err();
        let ImageError::Decoding(error) = error else {
            panic!("expected a decoding error, got {error}");
        };
        assert!(error
            .source()
            .unwrap()
            .downcast_ref::<jbig2_rs::DecodeError>()
            .is_some());
    }

    #[test]
    fn rejects_empty_pages_without_panicking() {
        let encoded = blank(0, 2, false);
        assert!(matches!(
            image::load_from_memory(&encoded),
            Err(ImageError::Unsupported(_))
        ));
    }

    struct Unreadable {
        length: u64,
    }

    impl Read for Unreadable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "test reader"))
        }
    }

    impl Seek for Unreadable {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            Ok(if matches!(position, SeekFrom::End(_)) {
                self.length
            } else {
                0
            })
        }
    }

    #[test]
    fn construction_is_lazy_and_io_errors_are_preserved() {
        let mut decoder = Jbig2Decoder::new(Unreadable { length: 13 });
        let Err(ImageError::IoError(error)) = decoder.prepare_image() else {
            panic!("expected original I/O error");
        };
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "test reader");
    }

    #[test]
    fn backend_stream_ceiling_is_enforced_before_reading() {
        let mut decoder = Jbig2Decoder::new(Unreadable {
            length: jbig2_rs::Limits::default().max_stream_bytes + 1,
        });
        decoder.set_limits(Limits::no_limits()).unwrap();
        assert!(matches!(
            decoder.prepare_image(),
            Err(ImageError::Limits(_))
        ));
    }

    #[test]
    #[should_panic]
    fn output_buffer_length_is_checked() {
        let mut decoder = Jbig2Decoder::new(Cursor::new(blank(9, 2, true)));
        decoder.read_image(&mut [0; 17]).unwrap();
    }
}
