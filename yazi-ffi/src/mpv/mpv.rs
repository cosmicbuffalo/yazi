use std::{ffi::{CStr, CString, c_int, c_void}, io, ptr, sync::{Condvar, Mutex}, time::Duration};

use super::{Library, MpvEvent, RenderParam};

const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;

const PARAM_API_TYPE: c_int = 1;
const PARAM_SW_SIZE: c_int = 17;
const PARAM_SW_FORMAT: c_int = 18;
const PARAM_SW_STRIDE: c_int = 19;
const PARAM_SW_POINTER: c_int = 20;

const UPDATE_FRAME: u64 = 1;

/// A libmpv player rendering into memory with its software renderer.
///
/// The client API (commands and properties) is thread-safe, while
/// [`Self::wait_frame`], [`Self::render`] and [`Self::next_event`] must be
/// called from a single thread.
pub struct Mpv {
	lib:    &'static Library,
	handle: *mut c_void,
	render: *mut c_void,
	ready:  Box<(Mutex<bool>, Condvar)>,
}

// SAFETY: libmpv's client API is thread-safe; the render API is confined to one
// thread by the contract documented above.
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Drop for Mpv {
	fn drop(&mut self) {
		unsafe {
			if !self.render.is_null() {
				(self.lib.render_free)(self.render);
			}
			(self.lib.terminate)(self.handle);
		}
	}
}

impl Mpv {
	pub fn new(options: &[(&str, &str)]) -> io::Result<Self> {
		let lib = Library::get()?;

		// libmpv refuses to start under a non-C numeric locale, which a library may
		// have picked up from `LANG` (e.g. `en_US.UTF-8`).
		unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
		let handle = unsafe { (lib.create)() };
		if handle.is_null() {
			return Err(io::Error::other("failed to create mpv instance"));
		}

		let mut me = Self { lib, handle, render: ptr::null_mut(), ready: Default::default() };
		for &(name, value) in
			[("vo", "libmpv"), ("terminal", "no"), ("config", "no")].iter().chain(options)
		{
			let (name, value) = (cstr(name)?, cstr(value)?);
			me.check(unsafe { (lib.set_option)(handle, name.as_ptr(), value.as_ptr()) })?;
		}
		me.check(unsafe { (lib.initialize)(handle) })?;

		let sw = c"sw";
		let mut params =
			[RenderParam { ty: PARAM_API_TYPE, data: sw.as_ptr() as *mut c_void }, RenderParam {
				ty:   0,
				data: ptr::null_mut(),
			}];
		let code = unsafe { (lib.render_create)(&mut me.render, handle, params.as_mut_ptr()) };
		me.check(code)?;

		let ready = &*me.ready as *const _ as *mut c_void;
		unsafe { (lib.render_callback)(me.render, Self::on_update, ready) };
		Ok(me)
	}

	pub fn command(&self, args: &[&str]) -> io::Result<()> {
		let args: Vec<_> = args.iter().map(|s| cstr(s)).collect::<io::Result<_>>()?;
		let mut ptrs: Vec<_> = args.iter().map(|s| s.as_ptr()).chain([ptr::null()]).collect();
		self.check(unsafe { (self.lib.command)(self.handle, ptrs.as_mut_ptr()) })
	}

	pub fn set(&self, name: &str, value: &str) -> io::Result<()> {
		let (name, value) = (cstr(name)?, cstr(value)?);
		self.check(unsafe { (self.lib.set_property)(self.handle, name.as_ptr(), value.as_ptr()) })
	}

	pub fn get_f64(&self, name: &str) -> Option<f64> {
		let mut v = 0f64;
		self.get(name, FORMAT_DOUBLE, &mut v as *mut _ as _).then_some(v)
	}

	pub fn get_i64(&self, name: &str) -> Option<i64> {
		let mut v = 0i64;
		self.get(name, FORMAT_INT64, &mut v as *mut _ as _).then_some(v)
	}

	pub fn get_bool(&self, name: &str) -> Option<bool> {
		let mut v: c_int = 0;
		self.get(name, FORMAT_FLAG, &mut v as *mut _ as _).then_some(v != 0)
	}

	/// Wakes up a thread blocked in [`Self::wait_frame`].
	pub fn wake(&self) {
		*self.ready.0.lock().unwrap() = true;
		self.ready.1.notify_one();
	}

	/// Blocks until mpv has something new, and returns whether a new video frame
	/// is ready to be rendered.
	pub fn wait_frame(&self, timeout: Duration) -> bool {
		let (lock, cvar) = &*self.ready;
		let guard = lock.lock().unwrap();
		let (mut guard, _) = cvar.wait_timeout_while(guard, timeout, |ready| !*ready).unwrap();
		*guard = false;
		drop(guard);

		unsafe { (self.lib.render_update)(self.render) & UPDATE_FRAME != 0 }
	}

	/// Renders the current frame as `rgb0` pixels into `buf`.
	pub fn render(&self, (w, h): (u32, u32), stride: usize, buf: &mut [u8]) -> io::Result<()> {
		assert!(buf.len() >= stride * h as usize);

		let mut size = [w as c_int, h as c_int];
		let mut stride = stride;
		let mut params = [
			RenderParam { ty: PARAM_SW_SIZE, data: size.as_mut_ptr().cast() },
			RenderParam { ty: PARAM_SW_FORMAT, data: c"rgb0".as_ptr() as *mut c_void },
			RenderParam { ty: PARAM_SW_STRIDE, data: (&mut stride as *mut usize).cast() },
			RenderParam { ty: PARAM_SW_POINTER, data: buf.as_mut_ptr().cast() },
			RenderParam { ty: 0, data: ptr::null_mut() },
		];
		self.check(unsafe { (self.lib.render)(self.render, params.as_mut_ptr()) })
	}

	/// Pops the next pending event, waiting up to `timeout` for one, and skipping
	/// the ones not modeled by [`MpvEvent`].
	pub fn next_event(&self, timeout: Duration) -> Option<MpvEvent> {
		let mut timeout = timeout.as_secs_f64();
		loop {
			let id = unsafe { *((self.lib.wait_event)(self.handle, timeout) as *const c_int) };
			timeout = 0.0;
			match MpvEvent::from_repr(id) {
				Some(MpvEvent::None) => return None,
				Some(e) => return Some(e),
				None => continue,
			}
		}
	}

	fn get(&self, name: &str, format: c_int, data: *mut c_void) -> bool {
		let Ok(name) = cstr(name) else { return false };
		unsafe { (self.lib.get_property)(self.handle, name.as_ptr(), format, data) >= 0 }
	}

	fn check(&self, code: c_int) -> io::Result<()> {
		if code >= 0 {
			return Ok(());
		}
		let s = unsafe { CStr::from_ptr((self.lib.error_string)(code)) };
		Err(io::Error::other(format!("mpv: {}", s.to_string_lossy())))
	}

	extern "C" fn on_update(data: *mut c_void) {
		let (lock, cvar) = unsafe { &*(data as *const (Mutex<bool>, Condvar)) };
		*lock.lock().unwrap() = true;
		cvar.notify_one();
	}
}

fn cstr(s: &str) -> io::Result<CString> { CString::new(s).map_err(io::Error::other) }
