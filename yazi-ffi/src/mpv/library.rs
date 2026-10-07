use std::{ffi::{c_char, c_int, c_void}, io, sync::OnceLock};

#[repr(C)]
pub(super) struct RenderParam {
	pub(super) ty:   c_int,
	pub(super) data: *mut c_void,
}

pub(super) struct Library {
	_lib: libloading::Library,

	pub(super) create:          unsafe extern "C" fn() -> *mut c_void,
	pub(super) initialize:      unsafe extern "C" fn(*mut c_void) -> c_int,
	pub(super) terminate:       unsafe extern "C" fn(*mut c_void),
	pub(super) set_option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
	pub(super) set_property: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
	pub(super) get_property:
		unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
	pub(super) command:         unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int,
	pub(super) wait_event:      unsafe extern "C" fn(*mut c_void, f64) -> *mut c_void,
	pub(super) error_string:    unsafe extern "C" fn(c_int) -> *const c_char,
	pub(super) render_create:
		unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut RenderParam) -> c_int,
	pub(super) render_callback:
		unsafe extern "C" fn(*mut c_void, extern "C" fn(*mut c_void), *mut c_void),
	pub(super) render_update:   unsafe extern "C" fn(*mut c_void) -> u64,
	pub(super) render:          unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int,
	pub(super) render_free:     unsafe extern "C" fn(*mut c_void),
}

impl Library {
	pub(super) fn get() -> io::Result<&'static Self> {
		static LIB: OnceLock<Result<Library, String>> = OnceLock::new();

		LIB
			.get_or_init(Self::load)
			.as_ref()
			.map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.as_str()))
	}

	fn load() -> Result<Self, String> {
		let names: &[&str] = if cfg!(target_os = "macos") {
			&[
				"libmpv.2.dylib",
				"/opt/homebrew/lib/libmpv.2.dylib",
				"/usr/local/lib/libmpv.2.dylib",
				"/opt/local/lib/libmpv.2.dylib",
			]
		} else if cfg!(windows) {
			&["libmpv-2.dll", "mpv-2.dll"]
		} else {
			&["libmpv.so.2", "libmpv.so"]
		};

		let lib = std::env::var_os("YAZI_LIBMPV")
			.into_iter()
			.chain(names.iter().map(Into::into))
			.find_map(|name| unsafe { libloading::Library::new(name) }.ok())
			.ok_or("libmpv not found, install mpv or set `YAZI_LIBMPV` to its path")?;

		macro_rules! sym {
			($name:literal) => {
				*unsafe { lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| e.to_string())?
			};
		}

		Ok(Self {
			create:          sym!("mpv_create"),
			initialize:      sym!("mpv_initialize"),
			terminate:       sym!("mpv_terminate_destroy"),
			set_option:      sym!("mpv_set_option_string"),
			set_property:    sym!("mpv_set_property_string"),
			get_property:    sym!("mpv_get_property"),
			command:         sym!("mpv_command"),
			wait_event:      sym!("mpv_wait_event"),
			error_string:    sym!("mpv_error_string"),
			render_create:   sym!("mpv_render_context_create"),
			render_callback: sym!("mpv_render_context_set_update_callback"),
			render_update:   sym!("mpv_render_context_update"),
			render:          sym!("mpv_render_context_render"),
			render_free:     sym!("mpv_render_context_free"),
			_lib:            lib,
		})
	}
}
