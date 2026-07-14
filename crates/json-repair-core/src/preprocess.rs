//! Input preprocessing before the repair state machine runs.
//!
//! Two independent transforms run before the repairer:
//!
//! - [`preamble::normalize_preamble`] — strips Markdown code fences, metatags,
//!   and wraps unbraced top-level keys (`"k": v` → `{"k": v}`).
//! - [`quote_fix::preprocess_json`] — fixes mixed-quote boundaries and colons
//!   embedded inside key strings.
//!
//! Both transforms return `Cow<str>` and avoid allocation when the input
//! needs no changes.

pub(crate) mod preamble;
pub(crate) mod quote_fix;

pub(crate) use preamble::normalize_preamble;
pub(crate) use quote_fix::preprocess_json;
