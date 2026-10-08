use anyhow::Result;
use yazi_adapter::ADAPTOR;
use yazi_macro::{render, succ};
use yazi_parser::player::StepForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Step;

impl Actor for Step {
	type Form = StepForm;

	const NAME: &str = "step";

	fn act(_: &mut Ctx, form: Self::Form) -> Result<Data> {
		let command = if form.frames < 0 { "frame-back-step" } else { "frame-step" };

		let mut done = false;
		for _ in 0..form.frames.unsigned_abs() {
			done |= ADAPTOR.video_command(&[command])?;
		}
		succ!(render!(done));
	}
}
