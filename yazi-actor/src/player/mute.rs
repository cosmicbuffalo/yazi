use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Mute;

impl Actor for Mute {
	type Form = VoidForm;

	const NAME: &str = "mute";

	fn act(_: &mut Ctx, _: Self::Form) -> Result<Data> {
		succ!(render!(ADAPTOR.video_command(&["cycle", "mute"])?));
	}
}
