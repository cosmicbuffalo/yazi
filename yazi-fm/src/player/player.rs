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

	/// Renders the key hints, wrapping them onto as many rows as the area needs.
	fn hints(area: Rect, buf: &mut Buffer) {
		const HINTS: [&str; 7] =
			["space pause", "h/l seek", "[/] speed", ",/. frame", "j/k volume", "m mute", "q stop"];

		let mut lines = vec![String::new()];
		for hint in HINTS {
			let line = lines.last_mut().unwrap();
			if line.is_empty() {
				line.push_str(hint);
			} else if line.chars().count() + 3 + hint.len() <= area.width as usize {
				line.push_str(" · ");
				line.push_str(hint);
			} else {
				lines.push(hint.to_owned());
			}
		}

		let dim = Style::new().add_modifier(Modifier::DIM);
		for (y, line) in (area.top()..area.bottom()).zip(lines) {
			Line::styled(line, dim).centered().render(Rect { y, height: 1, ..area }, buf);
		}
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

		let PlayerStatus { position, duration, paused, muted, volume, speed } = status;
		let head = format!("{} {} ", if paused { "⏸" } else { "▶" }, Self::clock(position));
		let tail = match (muted, (speed - 1.0).abs() > 1e-3) {
			(true, true) => format!(" {}  {speed:.2}×  mute", Self::clock(duration)),
			(true, false) => format!(" {}  mute", Self::clock(duration)),
			(false, true) => format!(" {}  {speed:.2}×  vol {volume}%", Self::clock(duration)),
			(false, false) => format!(" {}  vol {volume}%", Self::clock(duration)),
		};

		let fixed = head.chars().count() + tail.chars().count();
		let bar = (area.width as usize).saturating_sub(fixed);
		let done = if duration > 0.0 { (bar as f64 * position / duration).round() as usize } else { 0 };

		let dim = Style::new().add_modifier(Modifier::DIM);
		Line::from(vec![
			Span::raw(head),
			Span::raw("━".repeat(done.min(bar))),
			Span::styled("─".repeat(bar.saturating_sub(done)), dim),
			Span::styled(tail, dim),
		])
		.render(area, buf);

		if self.core.layer() == Layer::Player {
			Self::hints(
				Rect { y: area.y + 1, height: preview.bottom().saturating_sub(area.y + 1), ..area },
				buf,
			);
		}
	}
}
