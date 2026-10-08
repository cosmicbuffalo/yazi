use std::{path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, thread, time::{Duration, Instant}};

use anyhow::{Result, bail};
use image::{DynamicImage, RgbImage};
use ratatui_core::layout::Rect;
use tokio::runtime::Handle;
use yazi_emulator::Dimension;
use yazi_ffi::mpv::{Mpv, MpvEvent};
use yazi_macro::{emit, relay, warn};

use crate::{ADAPTOR, Image, drivers::Driver};

pub struct Player {
	mpv:    Arc<Mpv>,
	path:   PathBuf,
	video:  (u32, u32),
	muted:  AtomicBool,
	frame:  Arc<Mutex<Frame>>,
	stop:   Arc<AtomicBool>,
	thread: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Copy)]
struct Frame {
	area:   Rect,
	size:   (u32, u32),
	placed: bool,
	epoch:  u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerStatus {
	pub position: f64,
	pub duration: f64,
	pub paused:   bool,
	pub muted:    bool,
	pub volume:   i64,
	pub speed:    f64,
}

impl Drop for Player {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Relaxed);
		self.mpv.wake();
		if let Some(thread) = self.thread.take() {
			thread.join().ok();
		}

		// Tearing down mpv (audio output in particular) can take a while
		let mpv = self.mpv.clone();
		thread::spawn(move || drop(mpv));
	}
}

impl Player {
	pub(super) async fn open(path: PathBuf, max: Rect, muted: bool) -> Result<Self> {
		let p = path.clone();
		let (mpv, video) = tokio::task::spawn_blocking(move || Self::load(&p, muted)).await??;

		let size = Self::fit(video, max);
		let frame = Frame { area: Image::pixel_area(size, max), size, placed: false, epoch: 0 };
		Ok(Self {
			mpv: Arc::new(mpv),
			path,
			video,
			muted: AtomicBool::new(muted),
			frame: Arc::new(Mutex::new(frame)),
			stop: Default::default(),
			thread: None,
		})
	}

	pub(super) fn start(&mut self, driver: Driver) {
		let (mpv, frame, stop) = (self.mpv.clone(), self.frame.clone(), self.stop.clone());
		let rt = Handle::current();
		self.thread = Some(thread::spawn(move || Self::run(&mpv, &frame, &stop, driver, &rt)));
	}

	pub(super) fn area(&self) -> Rect { self.frame.lock().unwrap().area }

	pub(super) fn is(&self, path: &Path) -> bool { self.path == path }

	/// Moves the video into a new area, and redraws it from scratch.
	pub(super) fn relocate(&self, max: Rect) -> Rect {
		let size = Self::fit(self.video, max);
		let area = Image::pixel_area(size, max);
		let mut frame = self.frame.lock().unwrap();
		*frame = Frame { area, size, placed: false, epoch: frame.epoch + 1 };
		area
	}

	/// Applies the requested mute state if it differs from the last request, so
	/// that toggles made by the user in between are preserved.
	pub(super) fn remute(&self, muted: bool) {
		if self.muted.swap(muted, Ordering::Relaxed) != muted {
			self.mpv.set("mute", if muted { "yes" } else { "no" }).ok();
		}
	}

	pub fn command(&self, args: &[&str]) -> Result<()> { Ok(self.mpv.command(args)?) }

	pub fn status(&self) -> PlayerStatus {
		let mpv = &self.mpv;
		PlayerStatus {
			position: mpv.get_f64("time-pos").unwrap_or_default(),
			duration: mpv.get_f64("duration").unwrap_or_default(),
			paused:   mpv.get_bool("pause").unwrap_or_default(),
			muted:    mpv.get_bool("mute").unwrap_or_default(),
			volume:   mpv.get_i64("volume").unwrap_or_default(),
			speed:    mpv.get_f64("speed").unwrap_or(1.0),
		}
	}

	fn load(path: &Path, muted: bool) -> Result<(Mpv, (u32, u32))> {
		let mpv = Mpv::new(&[
			("hwdec", "auto-safe"),
			("keep-open", "yes"),
			("audio-display", "no"),
			("input-default-bindings", "no"),
			("mute", if muted { "yes" } else { "no" }),
		])?;
		mpv.set("sw-fast", "yes").ok();
		mpv.command(&["loadfile", &path.to_string_lossy()])?;

		let deadline = Instant::now() + Duration::from_secs(5);
		while Instant::now() < deadline {
			if let Some(MpvEvent::EndFile | MpvEvent::Shutdown) =
				mpv.next_event(Duration::from_millis(50))
			{
				bail!("mpv failed to play the file");
			}
			if let (Some(w @ 1..), Some(h @ 1..)) = (mpv.get_i64("dwidth"), mpv.get_i64("dheight")) {
				return Ok((mpv, (w as u32, h as u32)));
			}
		}
		bail!("No video stream found")
	}

	fn run(mpv: &Mpv, frame: &Mutex<Frame>, stop: &AtomicBool, driver: Driver, rt: &Handle) {
		let (mut buf, mut ticked) = (vec![], Instant::now());

		while !stop.load(Ordering::Relaxed) {
			let ready = mpv.wait_frame(Duration::from_millis(250));
			while let Some(event) = mpv.next_event(Duration::ZERO) {
				if event == MpvEvent::Shutdown {
					return;
				}
			}

			if ticked.elapsed() >= Duration::from_millis(250) {
				ticked = Instant::now();
				emit!(Call(relay!(player:tick)));
			}

			// Placeholder-based drivers survive overlapping widgets on their own,
			// while the others would draw on top of them.
			if !ready
				|| stop.load(Ordering::Relaxed)
				|| (driver != Driver::Kgp && ADAPTOR.collision.get())
			{
				continue;
			}

			let Frame { area, size: (w, h), placed, epoch } = *frame.lock().unwrap();
			let stride = (w as usize * 4).next_multiple_of(64);
			buf.resize(stride * h as usize, 0);
			if let Err(e) = mpv.render((w, h), stride, &mut buf) {
				warn!("[Player] Failed to render frame: {e}");
				continue;
			}

			let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
			for row in buf.chunks_exact(stride) {
				for px in row[..w as usize * 4].as_chunks::<4>().0 {
					rgb.extend_from_slice(&px[..3]);
				}
			}
			let Some(img) = RgbImage::from_raw(w, h, rgb) else { continue };

			match rt.block_on(driver.frame_show(DynamicImage::ImageRgb8(img), area, placed)) {
				Ok(()) => {
					let mut f = frame.lock().unwrap();
					f.placed |= f.epoch == epoch;
				}
				Err(e) => warn!("[Player] Failed to show frame: {e}"),
			}
		}
	}

	fn fit((vw, vh): (u32, u32), max: Rect) -> (u32, u32) {
		let (cw, ch) = Dimension::cell_size().unwrap_or((10.0, 20.0));
		let (mw, mh) = (max.width as f64 * cw, max.height as f64 * ch);

		let scale = (mw / vw as f64).min(mh / vh as f64).min(1.0);
		let w = ((vw as f64 * scale) as u32).max(2) & !1;
		let h = ((vh as f64 * scale) as u32).max(2) & !1;
		(w, h)
	}
}
