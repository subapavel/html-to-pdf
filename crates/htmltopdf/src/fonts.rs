//! Fonts the crate ships itself.
//!
//! Fonts are bundled bytes (never host fonts), so a document that names a face the caller did not
//! supply would otherwise lose its text silently. The built-in face is the last resort that keeps
//! text such as IBANs and variable symbols (`font-mono`) in the PDF. See ADR-0002.

/// JetBrains Mono Regular (SIL OFL 1.1, licence text in `fonts/OFL.txt`). Covers Latin Extended-A.
pub(crate) const BUILTIN_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");
