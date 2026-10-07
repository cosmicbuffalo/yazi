use strum::FromRepr;

#[derive(Clone, Copy, Debug, Eq, FromRepr, PartialEq)]
#[repr(i32)]
pub enum MpvEvent {
	None            = 0,
	Shutdown        = 1,
	EndFile         = 7,
	FileLoaded      = 8,
	VideoReconfig   = 17,
	Seek            = 20,
	PlaybackRestart = 21,
}
