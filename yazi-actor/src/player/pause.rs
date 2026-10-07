use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Pause;

impl Actor for Pause {
	type Form = VoidForm;

	const NAME: &str = "pause";

	fn act(_: &mut Ctx, _: Self::Form) -> Result<Data> {
		succ!(render!(ADAPTOR.video_command(&["cycle", "pause"])?));
	}
}
