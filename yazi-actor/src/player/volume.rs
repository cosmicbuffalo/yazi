use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::player::VolumeForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Volume;

impl Actor for Volume {
	type Form = VolumeForm;

	const NAME: &str = "volume";

	fn act(_: &mut Ctx, form: Self::Form) -> Result<Data> {
		succ!(render!(ADAPTOR.video_command(&["add", "volume", &form.delta.to_string()])?));
	}
}
