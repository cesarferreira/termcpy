//! Pixel rendering through the Kitty terminal graphics protocol.
use crate::{frame::Frame, geometry::Viewport};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

/// Removes both images the canvas alternates between.
pub const DELETE: &str = "\x1b_Ga=d,d=I,i=31,q=2;\x1b\\\x1b_Ga=d,d=I,i=32,q=2;\x1b\\";

/// The largest size that fits `max` with the frame's aspect ratio, never larger than the frame.
/// Pixels beyond what the terminal displays only cost bandwidth.
pub fn fit_pixels(frame: (u16, u16), max: (u32, u32)) -> (u32, u32) {
    let (width, height) = (u32::from(frame.0), u32::from(frame.1));
    if max.0 == 0 || max.1 == 0 || (width <= max.0 && height <= max.1) {
        return (width, height);
    }
    if u64::from(max.0) * u64::from(height) <= u64::from(max.1) * u64::from(width) {
        (max.0, (max.0 * height / width).max(1))
    } else {
        ((max.1 * width / height).max(1), max.1)
    }
}

/// Box-average `frame` down to `size`, applying the colour and invert options.
pub fn scale(frame: &Frame, size: (u32, u32), color: bool, invert: bool) -> Vec<u8> {
    let (src_w, src_h) = (usize::from(frame.width), usize::from(frame.height));
    let (dst_w, dst_h) = (size.0 as usize, size.1 as usize);
    let mut out = Vec::with_capacity(dst_w * dst_h * 3);
    for y in 0..dst_h {
        let y0 = y * src_h / dst_h;
        let y1 = ((y + 1) * src_h / dst_h).max(y0 + 1).min(src_h);
        for x in 0..dst_w {
            let x0 = x * src_w / dst_w;
            let x1 = ((x + 1) * src_w / dst_w).max(x0 + 1).min(src_w);
            let mut sum = [0_u32; 3];
            for row in y0..y1 {
                let line = &frame.rgb[(row * src_w + x0) * 3..(row * src_w + x1) * 3];
                for pixel in line.chunks_exact(3) {
                    sum[0] += u32::from(pixel[0]);
                    sum[1] += u32::from(pixel[1]);
                    sum[2] += u32::from(pixel[2]);
                }
            }
            let count = ((y1 - y0) * (x1 - x0)) as u32;
            let mut pixel = sum.map(|total| (total / count) as u8);
            if !color {
                let gray = ((299 * u32::from(pixel[0])
                    + 587 * u32::from(pixel[1])
                    + 114 * u32::from(pixel[2]))
                    / 1000) as u8;
                pixel = [gray; 3];
            }
            if invert {
                pixel = pixel.map(|channel| 255 - channel);
            }
            out.extend_from_slice(&pixel);
        }
    }
    out
}

/// The picture on screen. Each frame is placed as a new image before the previous one is
/// deleted, so the terminal never shows a gap between frames.
#[derive(Default)]
pub struct Canvas {
    shown: Option<u32>,
}

impl Canvas {
    /// Send `frame` scaled to at most `max` pixels, as zlib-compressed raw RGB, into the
    /// viewport's cell box. The cursor must already be at the viewport origin.
    pub fn draw(
        &mut self,
        out: &mut impl Write,
        frame: &Frame,
        view: Viewport,
        max: (u32, u32),
        color: bool,
        invert: bool,
    ) -> Result<()> {
        let size = fit_pixels((frame.width, frame.height), max);
        let pixels = scale(frame, size, color, invert);
        // A phone UI is mostly flat colour, so the fastest zlib level already shrinks it a lot.
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&pixels)?;
        let encoded = STANDARD.encode(encoder.finish()?);
        let id = if self.shown == Some(31) { 32 } else { 31 };
        let chunks = encoded.as_bytes().chunks(4096);
        let count = chunks.len();
        for (index, chunk) in chunks.enumerate() {
            if index == 0 {
                write!(
                    out,
                    "\x1b_Ga=T,f=24,o=z,s={},v={},i={id},p=1,q=2,C=1,c={},r={},",
                    size.0, size.1, view.cols, view.rows
                )?;
            } else {
                write!(out, "\x1b_Gq=2,")?;
            }
            write!(out, "m={};", u8::from(index + 1 < count))?;
            out.write_all(chunk)?;
            out.write_all(b"\x1b\\")?;
        }
        if let Some(old) = self.shown.replace(id) {
            write!(out, "\x1b_Ga=d,d=I,i={old},q=2;\x1b\\")?;
        }
        Ok(())
    }

    /// Forget the shown image after the caller deleted it, for example with [`DELETE`].
    pub fn reset(&mut self) {
        self.shown = None;
    }
}

/// Probe before crossterm starts reading events, so protocol replies cannot become keys.
#[cfg(unix)]
pub fn check_terminal(cancelled: &std::sync::atomic::AtomicBool) -> Result<()> {
    use std::{
        io::{self},
        os::fd::AsRawFd,
        time::{Duration, Instant},
    };
    crossterm::terminal::enable_raw_mode()?;
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _restore = Restore;
    let mut output = io::stdout().lock();
    output.write_all(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c")?;
    output.flush()?;
    let deadline = Instant::now() + Duration::from_millis(700);
    let input = io::stdin().lock();
    let mut response = Vec::new();
    while Instant::now() < deadline && response.len() < 4096 {
        ensure!(
            !cancelled.load(std::sync::atomic::Ordering::Relaxed),
            "Connection cancelled"
        );
        let mut descriptor = libc::pollfd {
            fd: input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // The descriptor belongs to the locked stdin and remains valid throughout poll.
        let available = unsafe { libc::poll(&mut descriptor, 1, 20) };
        if available <= 0 {
            continue;
        }
        let mut bytes = [0_u8; 512];
        // Read the descriptor directly: std::io buffering would hide unread bytes from poll.
        let count =
            unsafe { libc::read(input.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if count == 0 {
            break;
        }
        response.extend_from_slice(&bytes[..count as usize]);
        // Wait for device attributes too, draining the entire probe's replies.
        if response.last() == Some(&b'c') && response.windows(3).any(|w| w == b"\x1b[?") {
            break;
        }
    }
    ensure!(
        response
            .windows(b"\x1b_Gi=31;OK\x1b\\".len())
            .any(|w| w == b"\x1b_Gi=31;OK\x1b\\"),
        "This terminal did not confirm Kitty graphics support. Run --render graphics in Kitty or a compatible terminal, or use --render blocks."
    );
    Ok(())
}

#[cfg(not(unix))]
pub fn check_terminal(_cancelled: &std::sync::atomic::AtomicBool) -> Result<()> {
    anyhow::bail!(
        "Graphics support detection currently requires Unix. Use --render blocks on this platform."
    )
}
