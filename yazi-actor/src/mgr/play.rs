use anyhow::Result;
use yazi_macro::succ;
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct Play;

impl Actor for Play {
	type Form = VoidForm;

	const NAME: &str = "play";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		if cx.hovered().is_none() {
			succ!();
		}

		cx.tab_mut().preview.play = true;
		act!(mgr:peek, cx, true)
	}
}
