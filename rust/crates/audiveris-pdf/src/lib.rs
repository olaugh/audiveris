// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading a PDF the way Audiveris does, which means the way PDFBox does.
//!
//! Audiveris never extracts an image from a PDF. `ImageLoading.PdfboxLoader`
//! calls `PDFRenderer.renderImageWithDPI(page, 300, ImageType.GRAY)` under
//! `ANTIALIAS_OFF` and `INTERPOLATION_BICUBIC`, so the input to every later
//! stage is a *rendered page*, and the port has to reproduce a rasterizer
//! rather than a set of decoders. Binarization runs on the result and GRID runs
//! on that, so a sample that is one count off can flip a pixel and move a staff
//! line.
//!
//! The layers, in the order they were built:
//!
//! - [`transform`] -- Java2D's bicubic image transform, sample for sample. This
//!   was the uncertain piece, so it came first: everything else is worth
//!   nothing without it. Verified against Java on 112 synthetic cases and on
//!   real page renders.
//! - [`object`], [`lexer`] -- the file syntax: objects, dictionaries, streams.
//! - [`document`] -- cross-reference tables and streams, object streams, the
//!   trailer chain, and the page tree with its inherited attributes.
//! - [`flate`], [`filter`] -- the general stream filters and their predictors.
//! - [`ccitt`] -- `CCITTFaxDecode`, which 93 of the corpus's 189 images use.
//!
//! Still to come: `JBIG2Decode`, which carries 95 of the remaining images, and
//! the content-stream plumbing that composes a page's draws.
//!
//! # Verification
//!
//! `oracle/java/PdfPageProbe.java` renders the corpus through PDFBox and pins
//! three things per page: the structure and filter chain of each image, an
//! FNV-1a-64 of the filtered stream bytes, and an FNV-1a-64 of the rendered
//! page. Each layer here is graded against the corresponding half of that file,
//! so a decoder can be finished and checked before the layer above it exists.
//!
//! No non-Rust dependency is linked in: as with the JPEG decoder, matching
//! Java's output means reproducing the library Java actually runs, and linking
//! a different implementation of the same standard is how the JPEG work lost a
//! day to libjpeg 6b versus turbo.

#![forbid(unsafe_code)]

pub mod ccitt;
pub mod document;
pub mod error;
pub mod filter;
pub mod flate;
pub mod jbig2;
pub mod lexer;
pub mod object;
pub mod transform;

pub use document::{Document, Page, Rectangle};
pub use error::{Error, Result};
pub use object::{Dictionary, Name, Object, Reference, Stream};
pub use transform::{GrayImage, Placement, draw_bicubic, scale_bicubic};
