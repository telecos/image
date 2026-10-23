# Experimental JBIG2 integration

This PoC integrates the sibling `../jbig2_rs` checkout through the opt-in
`jbig2` Cargo feature. It is not enabled by `default-formats`, provides no
encoder, and is not a publication-ready dependency or a conformance claim.
The backend remains under development and is not suitable for untrusted
production documents.

Both the ordinary backend and this integration support Rust **1.88.0**, matching
image's declared MSRV. The backend's 1.89.0 development compiler and its separate
Verus, fuzz and coverage pins are unchanged. The sibling path dependency must
be replaced with a released/pinned dependency before upstreaming.

The JPEG 2000 PoC is saved independently on local image branch `poc/jpeg2000`.
This exercise is on `poc/jbig2`, based on image's original `main`, not on the
JPEG 2000 changes. Backend MSRV changes are on `poc/image-msrv-1.88` in the
sibling JBIG2 repository.

## Decoder contract

- Normal `open`, `load_from_memory`, and `ImageReaderOptions` APIs dispatch to
  `image::codecs::jbig2::Jbig2Decoder`.
- Extensions `jb2` and `jbig2`, and standalone magic bytes
  `97 4a 42 32 0d 0a 1a 0a`, are recognized even with decoding disabled.
  No JBIG2-specific MIME mapping is added; `to_mime_type` uses the
  `application/octet-stream` fallback.
- Exactly one nonempty page is supported, in sequential or random-access file
  organization, with a known or unknown page count. Unknown-height striped
  pages use the final height established by the backend's inspection.
- Multi-page documents are rejected rather than silently selecting a page.
  Empty pages are also rejected. Embedded/PDF streams and reusable globals
  require the backend API, not this standalone adapter.
- Output is interleaved **L8**, with black **0** and white **255**. Backend bits
  are most-significant-bit first, with one meaning black. Row padding is not
  exposed as pixels. Colour extensions remain unsupported.
- Construction performs no I/O. Preparation buffers the seek-bounded encoded
  input and inspects its envelope without decoding pixels. It checks image
  dimensions and reserves the expanded L8 output before pixel decoding.
- The encoded input and output count against image's allocation budget. The
  remaining budget constrains backend managed-memory and retained-state limits.
  Backend hard ceilings remain active with `Limits::no_limits()`. Accounting
  is best-effort, not a guarantee of aggregate process peak memory.
- Read/seek errors remain I/O errors. Allocation and relevant size limits map
  to image limits; unsupported features remain unsupported errors. Other
  backend failures retain their typed source in a decoding error.

## Official reference-pixel checks

The approved external assets are the electronic conformance data attached to
[ITU-T T.88 (08/2018)](https://www.itu.int/rec/T-REC-T.88-201808-I/en).
The corresponding reference files are **BMP**, not the TIFF files used in the
JPEG 2000 exercise.

The backend's [provenance record](../../jbig2_rs/source-provenance.json) records
approval for internal/private-CI execution, but no redistribution permission.
No standards text, sample software, official streams or reference bitmaps are
committed. The preparation script extracts only the 26 approved data files into
ignored image `target/` storage.

Pinned SHA-256 identities:

| Asset | SHA-256 |
| ----- | ------- |
| ITU outer software package | `e2a3c458c4d28d5a835c0815fd9b23e197dbc1b83fe81d8e8d8b2ac413d8f22f` |
| Inner conformance ZIP | `fe3de9056d94b20db3a6ab17032c621a7f05b6ecb1db84b4b58d0825fd6b7033` |
| Sibling per-file manifest | `e9ee986db84fdfe960d56dedb843d3c16ea3e12d2a2f97f6cf56e96f55372ff1` |

The preparation script accepts either ZIP, authenticates the conformance
archive and sibling manifest, and verifies every file's size and hash before
writing any assets. It does not extract the outer package's standards or sample
software. Obtain the package from ITU for authorized testing, then run from
this image checkout:

```sh
python3 tests/jbig2/prepare_itu_vectors.py /path/to/T.88-2018-package.zip

JBIG2_ITU_VECTOR_ROOT="$PWD/target/jbig2-conformance/files/JBIG2_ConformanceData-A20180829" \
  cargo +1.88.0 test --locked --release --no-default-features \
  --features jbig2,bmp --test jbig2_conformance -- --ignored
```

No network access occurs during Cargo tests. The tests are ignored by default
because assets are external. Explicit execution without the environment
variable or required files fails, not skips. BMP support is needed only to read
the independent references, not to decode JBIG2.

| Official stream | Official reference | Result on Rust 1.88 |
| --------------- | ------------------ | ------------------- |
| `F01_200_TT9.jb2` | `F01_200_TT9_TT00.bmp` | Exact pixels, 1728 by 2339 |
| `F01_200_TT10.jb2` | `F01_200_TT10_TT00.bmp` | Exact pixels, 1728 by 2339 |

Both files are decoded through image's normal file API. The tests assert L8
output, dimensions, sample count, and every pixel. Each L8 sample is replicated
into RGB and compared with the official BMP decoded through image's independent
BMP codec, which applies its palette and row orientation. There is no error
tolerance, regenerated expected image, or visual-only comparison.

### Broader conformance gaps

The full ten-stream backend runner was also executed with actual Rust 1.88.
It reproduced the existing baseline:

- **2 exact pixel passes**, the cases above.
- **7 unresolved official positives**, still failures, not accepted rejections.
- **1 unsupported colour-extension case**.

The full runner exits **nonzero**. No parser acceptance, entropy interpretation,
expected pixels, or source-bound diagnoses were relaxed to produce passes.
The backend's [seven-case review](../../jbig2_rs/validation/conformance/seven-rejection-review.json)
requires normative/owner adjudication before stronger claims. The image adapter
also intentionally excludes the multi-page `codeStreamTest1_TT1` document.

To reproduce the full backend result, run from the sibling backend checkout
after preparing the assets:

```sh
cargo +1.88.0 build --locked --release --example decode_vector
python3 validation/conformance/run_itu.py \
  --vectors ../image/target/jbig2-conformance/files \
  --decoder target/release/examples/decode_vector \
  --expected-differences validation/conformance/expected-differences.json \
  --output target/conformance/image-evaluation-rust-1.88.json
```

The full runner checks per-file hashes. If you also have the inner conformance
ZIP, optionally pass `--archive /path/to/conformance.zip` to authenticate it
again. Preparation from the outer ZIP does not write the inner ZIP separately.

## Rust 1.88 qualification

Validated on Linux using actual `rustc 1.88.0 (6b00bc388 2025-06-23)` and
Cargo 1.88.0, without decoder source changes or dependency downgrades:

- Backend: **402 executed Rust tests passed**, 14 intentionally ignored;
  documentation tests and the release all-targets check passed.
- The isolated 24-file package built and passed its unit/doc tests with a fresh,
  offline Cargo home. A separate downstream consumer checked exact generated
  pixels across the public APIs, with no proof files or dependencies.
- Image: **14 integration tests passed** with minimal and default features;
  **332 existing library tests** and **2 feature-disabled tests** passed.
- Both official BMP pixel comparisons above passed in release mode.
- Backend strict Clippy and 27 related distribution/workflow Python tests
  passed; the immutable workflow policy check passed.
- Image Clippy passed with only `clippy::uninlined_format_args` allowed on the
  command line. Unqualified strict Clippy on Rust 1.88 finds four pre-existing
  instances in unchanged image code; no source-level suppressions were added.

Reproduce ordinary builds without changing global defaults:

```sh
rustup toolchain install 1.88.0 --profile minimal

# From the sibling jbig2_rs checkout:
cargo +1.88.0 test --locked --all-targets
cargo +1.88.0 test --locked --doc
cargo +1.88.0 check --locked --release --all-targets
# This existing validation tool requires Python 3.11+:
python3 tools/check_distribution.py --toolchain 1.88.0

# From this image checkout:
cargo +1.88.0 test --locked --no-default-features --features jbig2 --test jbig2
cargo +1.88.0 test --locked --features jbig2 --lib --test jbig2
cargo +1.88.0 test --locked --no-default-features --test jbig2
```

Backend distribution CI now selects Rust 1.88 and stable on all three native
platforms; the development/verification pins are unchanged. Image's existing
MSRV CI only covers default features, not this local sibling-path integration.
The integration still needs both checkouts locally.

Hosted CI, other platforms, Verus, fuzzing and coverage gates were not rerun for
this PoC. Existing proof/conformance/readiness gaps remain; these results do not
extend certification or production-readiness claims.
