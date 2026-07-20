//! Cached PATH probes for optional preview binaries (`FFmpeg`, Poppler `pdftoppm`, `MuPDF` `mutool`, `resvg`).
//! Probed once per process via [`std::sync::OnceLock`] so the Settings UI does not spawn every frame.

use std::process::Command;
use std::sync::OnceLock;

struct Cached {
    ffmpeg: bool,
    poppler_pdftoppm: bool,
    mutool: bool,
    resvg: bool,
}

static PROBE: OnceLock<Cached> = OnceLock::new();

fn cached() -> &'static Cached {
    PROBE.get_or_init(|| Cached {
        ffmpeg: probe_ffmpeg(),
        poppler_pdftoppm: probe_pdftoppm(),
        mutool: probe_mutool(),
        resvg: probe_resvg(),
    })
}

/// `ffmpeg` on `PATH` and responds to `-version`.
#[must_use]
pub fn ffmpeg_available() -> bool {
    cached().ffmpeg
}

/// Poppler [`pdftoppm`] on `PATH` (same install as `pdfinfo`).
#[must_use]
pub fn poppler_pdftoppm_available() -> bool {
    cached().poppler_pdftoppm
}

/// `MuPDF` [`mutool`] on `PATH`.
#[must_use]
pub fn mutool_available() -> bool {
    cached().mutool
}

/// [`resvg`] on `PATH` (SVG raster preview).
#[must_use]
pub fn resvg_available() -> bool {
    cached().resvg
}

fn probe_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn probe_pdftoppm() -> bool {
    Command::new("pdftoppm").arg("-v").output().is_ok()
}

fn probe_mutool() -> bool {
    Command::new("mutool").arg("-v").output().is_ok()
}

fn probe_resvg() -> bool {
    Command::new("resvg")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}
