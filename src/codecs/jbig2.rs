//! Experimental standalone, single-page JBIG2 decoding using `jbig2_rs`.
//!
//! Packed bi-level pixels are expanded to L8: one is black (0), zero is white
//! (255). Embedded/PDF streams, multi-page documents and colour extensions are
//! not supported by this adapter. The backend remains under development and
//! retains its hardened resource ceilings even with image-level limits disabled.

use std::io::{Read, Seek, SeekFrom};

use jbig2_rs::{DecodeError, Decoder, LimitKind, PageInfo};

use crate::error::{
    DecodingError, LimitError, LimitErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::io::{DecodedImageAttributes, DecoderPreparedImage};
use crate::{ColorType, ImageDecoder, ImageError, ImageFormat, ImageResult, Limits};

/// A decoder for standalone JBIG2 files containing exactly one page.
///
/// Construction performs no I/O. Preparation buffers the bounded input and
/// inspects page metadata without decoding pixels. Use the backend directly
/// for embedded/PDF streams, shared globals or multi-page documents.
pub struct Jbig2Decoder<R> {
    reader: R,
    input: Option<Vec<u8>>,
    page: Option<PageInfo>,
    limits: Limits,
}

impl<R: Read + Seek> Jbig2Decoder<R> {
    /// Creates a decoder without consuming input.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            input: None,
            page: None,
            limits: Limits::no_limits(),
        }
    }

    fn resource_limits(&self, limits: &Limits) -> ImageResult<jbig2_rs::Limits> {
        let mut budget = limits.clone();
        if let Some(input) = &self.input {
            budget.reserve_usize(input.len())?;
        }
        if let Some(page) = self.page {
            budget.reserve_buffer(page.width(), page.height(), ColorType::L8)?;
        }
        let mut resource = jbig2_rs::Limits::default();
        if let Some(bytes) = budget.max_alloc {
            resource.max_decoder_managed_memory_bytes =
                bytes.min(resource.max_decoder_managed_memory_bytes);
            resource.max_retained_state_bytes = bytes.min(resource.max_retained_state_bytes);
        }
        Ok(resource)
    }

    fn ensure_input(&mut self) -> ImageResult<()> {
        if self.input.is_some() {
            return Ok(());
        }
        let start = self.reader.stream_position()?;
        let end = self.reader.seek(SeekFrom::End(0))?;
        self.reader.seek(SeekFrom::Start(start))?;
        let length = end
            .checked_sub(start)
            .ok_or_else(|| malformed("input ends before the current position"))?;
        if length > jbig2_rs::Limits::default().max_stream_bytes {
            return Err(memory_limit());
        }
        self.limits.clone().reserve(length)?;
        let length = usize::try_from(length).map_err(|_| memory_limit())?;
        let mut input = Vec::new();
        input
            .try_reserve_exact(length)
            .map_err(|_| memory_limit())?;
        input.resize(length, 0);
        self.reader.read_exact(&mut input)?;
        self.input = Some(input);
        Ok(())
    }

    fn ensure_header(&mut self) -> ImageResult<()> {
        if self.page.is_some() {
            return Ok(());
        }
        self.ensure_input()?;
        let input = self.input.as_deref().expect("input has been buffered");
        let info = Decoder::new(self.resource_limits(&self.limits)?)
            .inspect(input)
            .map_err(decoder_error)?;
        if info.pages().len() != 1 {
            return Err(unsupported("only single-page JBIG2 files are supported"));
        }
        let page = info.pages()[0];
        if page.width() == 0 || page.height() == 0 {
            return Err(unsupported("empty JBIG2 pages are not supported"));
        }
        let mut budget = self.limits.clone();
        budget.reserve_usize(input.len())?;
        budget.reserve_buffer(page.width(), page.height(), ColorType::L8)?;
        self.page = Some(page);
        Ok(())
    }
}

impl<R: Read + Seek> ImageDecoder for Jbig2Decoder<R> {
    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::LimitSupport::default())?;
        self.resource_limits(&limits)?;
        self.limits = limits;
        Ok(())
    }

    fn prepare_image(&mut self) -> ImageResult<DecoderPreparedImage> {
        self.ensure_header()?;
        let page = self.page.expect("page has been inspected");
        Ok(DecoderPreparedImage::new(
            page.width(),
            page.height(),
            ColorType::L8,
        ))
    }

    fn read_image(&mut self, buf: &mut [u8]) -> ImageResult<DecodedImageAttributes> {
        let layout = self.prepare_image()?;
        assert_eq!(u64::try_from(buf.len()), Ok(layout.total_bytes()));
        let document = Decoder::new(self.resource_limits(&self.limits)?)
            .decode(self.input.as_deref().expect("input has been buffered"))
            .map_err(decoder_error)?;
        let page = self.page.expect("page has been inspected");
        let [decoded] = document.pages() else {
            return Err(malformed("decoded page count differs from metadata"));
        };
        let bitmap = decoded.bitmap();
        if decoded.number() != page.number()
            || bitmap.width() != page.width()
            || bitmap.height() != page.height()
        {
            return Err(malformed("decoded page differs from metadata"));
        }
        let width = usize::try_from(page.width()).map_err(|_| memory_limit())?;
        let height = usize::try_from(page.height()).map_err(|_| memory_limit())?;
        let stride = width.div_ceil(8);
        if bitmap.stride() != stride
            || bitmap.bytes().len() != stride.checked_mul(height).ok_or_else(memory_limit)?
        {
            return Err(malformed("invalid decoded bitmap layout"));
        }
        for (packed, output) in bitmap
            .bytes()
            .chunks_exact(stride)
            .zip(buf.chunks_exact_mut(width))
        {
            for (x, sample) in output.iter_mut().enumerate() {
                *sample = if packed[x / 8] & (0x80 >> (x % 8)) == 0 {
                    255
                } else {
                    0
                };
            }
        }
        Ok(DecodedImageAttributes::default())
    }
}

fn memory_limit() -> ImageError {
    ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
}

fn malformed(message: &'static str) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormat::Jbig2.into(), message))
}

fn unsupported(message: impl Into<String>) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        ImageFormat::Jbig2.into(),
        UnsupportedErrorKind::GenericFeature(message.into()),
    ))
}

fn decoder_error(error: DecodeError) -> ImageError {
    match &error {
        DecodeError::Allocation { .. } => memory_limit(),
        DecodeError::LimitExceeded(limit) => match limit.kind {
            LimitKind::Width | LimitKind::Height | LimitKind::PagePixels => {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError))
            }
            LimitKind::Memory
            | LimitKind::StreamBytes
            | LimitKind::DecodedBitmapBytes
            | LimitKind::RetainedState => memory_limit(),
            _ => ImageError::Decoding(DecodingError::new(ImageFormat::Jbig2.into(), error)),
        },
        DecodeError::Unsupported(_) => unsupported(error.to_string()),
        _ => ImageError::Decoding(DecodingError::new(ImageFormat::Jbig2.into(), error)),
    }
}
