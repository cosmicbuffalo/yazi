use ratatui_core::{buffer::Buffer, layout::Rect, style::{Modifier, Style}, text::{Line, Span}, widgets::Widget};
use yazi_adapter::{ADAPTOR, PlayerStatus};
use yazi_config::LAYOUT;
use yazi_core::Core;
use yazi_shared::Layer;

pub(crate) struct Player<'a> {
	core: &'a Core,
}

impl<'a> Player<'a> {
	pub(crate) fn new(core: &'a Core) -> Self { Self { core } }

	fn clock(secs: f64) -> String {
		let secs = secs.max(0.0) as u64;
		let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
		if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
	}
}

impl Widget for Player<'_> {
	fn render(self, _: Rect, buf: &mut Buffer) {
		let (Some(status), Some(shown)) = (ADAPTOR.video_status(), ADAPTOR.shown_area()) else {
			return;
		};

		let preview = LAYOUT.get().preview;
		if shown.bottom() >= preview.bottom() {
			return;
		}
		let area = Rect { x: preview.x, y: shown.bottom(), width: preview.width, height: 1 };

		let PlayerStatus { position, duration, paused, muted, volume } = status;
		let icon = if paused { "⏸" } else { "▶" };
		let sound = if muted { "mute".to_owned() } else { format!("vol {volume}%") };
		let (now, end) = (Self::clock(position), Self::clock(duration));

		let fixed = icon.len().min(1) + now.len() + end.len() + sound.len() + 6;
		let bar = (area.width as usize).saturating_sub(fixed);
		let done = if duration > 0.0 { (bar as f64 * position / duration).round() as usize } else { 0 };

		let dim = Style::new().add_modifier(Modifier::DIM);
		Line::from(vec![
			Span::raw(format!("{icon} {now} ")),
			Span::raw("━".repeat(done.min(bar))),
			Span::styled("─".repeat(bar.saturating_sub(done)), dim),
			Span::raw(format!(" {end}  ")),
			Span::styled(sound, dim),
		])
		.render(area, buf);

		if self.core.layer() == Layer::Player && area.y + 1 < preview.bottom() {
			let hints = "space pause · h/l seek · j/k volume · m mute · q stop";
			Line::styled(hints, dim).centered().render(Rect { y: area.y + 1, ..area }, buf);
		}
	}
}
