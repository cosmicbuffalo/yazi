use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::player::SpeedForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Speed;

impl Actor for Speed {
	type Form = SpeedForm;

	const NAME: &str = "speed";

	fn act(_: &mut Ctx, form: Self::Form) -> Result<Data> {
		let done = if form.reset {
			ADAPTOR.video_command(&["set", "speed", "1"])?
		} else if let Some(factor) = form.factor {
			ADAPTOR.video_command(&["multiply", "speed", &factor.to_string()])?
		} else {
			false
		};
		succ!(render!(done));
	}
}
