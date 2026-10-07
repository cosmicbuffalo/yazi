use std::{fmt::{self, Debug}, path::PathBuf, sync::{Mutex, OnceLock}};

use anyhow::{Result, bail};
use ratatui_core::layout::Rect;
use yazi_emulator::EMULATOR;
use yazi_shim::cell::SyncCell;
use yazi_widgets::clear::ClearInventory;

use crate::{ADAPTOR, Player, PlayerStatus, drivers::{Driver, Drivers}};

#[derive(Default)]
pub struct Adapter {
	driver:        OnceLock<Driver>,
	shown:         SyncCell<Option<Rect>>,
	player:        Mutex<Option<Player>>,
	pub collision: SyncCell<bool>,
}

impl Debug for Adapter {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.driver.get() {
			Some(driver) => driver.fmt(f),
			None => f.write_str("Pending"),
		}
	}
}

impl Adapter {
	pub async fn image_show<P>(&self, path: P, max: Rect) -> Result<Rect>
	where
		P: Into<PathBuf>,
	{
		self.driver().await.image_show(path, max).await
	}

	pub fn image_hide(&self) -> Result<()> {
		drop(self.player.lock().unwrap().take());

		let Some(area) = self.shown.replace(None) else { return Ok(()) };
		match self.driver.get() {
			Some(driver) => driver.image_erase(area),
			None => Ok(()),
		}
	}

	pub async fn video_play<P>(&self, path: P, max: Rect, muted: bool) -> Result<Rect>
	where
		P: Into<PathBuf>,
	{
		let driver = self.driver().await;
		if !driver.supports_video() {
			bail!("Video playback is not supported by the `{driver}` image adapter");
		} else if max.is_empty() {
			return Ok(Rect::default());
		}

		let path = path.into();
		if let Some(player) = &*self.player.lock().unwrap()
			&& player.is(&path)
		{
			player.remute(muted);
			let (old, new) = (player.area(), player.relocate(max));
			if old != new {
				driver.image_erase(old)?;
			}
			self.shown_store(new);
			return Ok(new);
		}

		let mut player = Player::open(path, max, muted).await?;
		self.image_hide()?;

		let area = player.area();
		self.shown_store(area);
		player.start(driver);
		*self.player.lock().unwrap() = Some(player);
		Ok(area)
	}

	pub fn video_command(&self, args: &[&str]) -> Result<bool> {
		match &*self.player.lock().unwrap() {
			Some(player) => player.command(args).map(|()| true),
			None => Ok(false),
		}
	}

	pub fn video_playing(&self) -> bool { self.player.lock().unwrap().is_some() }

	pub fn video_status(&self) -> Option<PlayerStatus> {
		self.player.lock().unwrap().as_ref().map(Player::status)
	}

	async fn driver(&self) -> Driver {
		let probe = &EMULATOR.probe;
		probe.wait(probe.id.get()).await;

		*self.driver.get_or_init(|| {
			let driver = Drivers::matches(&EMULATOR);
			driver.start();
			driver
		})
	}

	pub fn shown_area(&self) -> Option<Rect> { self.shown.get() }

	pub(super) fn shown_store(&self, area: Rect) { self.shown.set(Some(area)); }
}

inventory::submit! {
	ClearInventory {
		clear: |area| {
			let overlap = area.intersection(ADAPTOR.shown.get()?);
			if overlap.area() == 0 {
				return None;
			}

			ADAPTOR.driver.get()?.image_erase(overlap).ok();
			ADAPTOR.collision.set(true);
			Some(overlap)
		},
	}
}
