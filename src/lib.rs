//! Draw a pixel-art galaxy from a GitHub account's public activity.
//!
//! The pipeline runs one way, and each module owns one stage of it:
//!
//! 1. [`collect`] gathers an [`history::AccountSnapshot`] through the GitHub CLI.
//! 2. [`history`] reads and writes snapshots and derives language shares and weekly
//!    participation from them.
//! 3. [`galaxy`] turns a snapshot into a [`galaxy::GalaxyDevelopment`]: identity,
//!    language colors, growth, streaks, and recent activity.
//! 4. [`render`] draws a [`render::GalaxyFrame`] in a [`colorway::Colorway`] on a
//!    [`theme::Theme`] background, and [`output`] encodes it as PNG or a looping GIF,
//!    with a preview page.
//!
//! Everything after collection is deterministic: it reads no clock, performs no I/O, and
//! mutates no input. Callers own files and publication. The `gitlaxy` command composes
//! these stages; the repository's getting-started guide covers the command and the GitHub
//! Action. Breaking changes to this API bump the major version.
#![warn(missing_docs)]

pub mod collect;
pub mod colorway;
pub mod galaxy;
pub mod history;
pub mod motion;
pub mod output;
pub mod pigment;
pub mod render;
pub mod theme;
