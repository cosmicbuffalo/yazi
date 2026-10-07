use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::player::SeekForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Seek;

impl Actor for Seek {
	type Form = SeekForm;

	const NAME: &str = "seek";

	fn act(_: &mut Ctx, form: Self::Form) -> Result<Data> {
		succ!(render!(ADAPTOR.video_command(&["seek", &form.offset.to_string(), "relative+exact"])?));
	}
}
