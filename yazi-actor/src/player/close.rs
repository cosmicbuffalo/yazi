use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_config::YAZI;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct Close;

impl Actor for Close {
	type Form = VoidForm;

	const NAME: &str = "close";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		cx.tab_mut().preview.play = false;
		if YAZI.preview.video_autoplay {
			ADAPTOR.video_command(&["set", "mute", "yes"])?;
			succ!(render!());
		}

		cx.tab_mut().preview.reset_image();
		act!(mgr:peek, cx, true)
	}
}
