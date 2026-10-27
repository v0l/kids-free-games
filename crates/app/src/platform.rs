use kg_core::safe::Insets;

#[cfg(target_os = "android")]
mod imp {
    use super::Insets;
    use jni::objects::{JObject, JPrimitiveArray};
    use jni::JavaVM;

    fn app() -> Option<&'static bevy::android::android_activity::AndroidApp> {
        bevy::android::ANDROID_APP.get()
    }

    fn with_activity<R>(f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> Option<R>) -> Option<R> {
        let app = app()?;
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let out = f(&mut env, &activity);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return None;
        }
        out
    }

    pub fn insets_px() -> Insets {
        with_activity(|env, act| {
            let arr = env
                .call_method(act, "safeAreaInsets", "()[I", &[])
                .ok()?
                .l()
                .ok()?;
            let arr = JPrimitiveArray::<jni::sys::jint>::from(arr);
            let mut out = [0i32; 4];
            env.get_int_array_region(&arr, 0, &mut out).ok()?;
            Some(Insets {
                left: out[0] as f32,
                top: out[1] as f32,
                right: out[2] as f32,
                bottom: out[3] as f32,
            })
        })
        .unwrap_or_default()
    }

    pub fn units_per_mm_px() -> Option<f32> {
        with_activity(|env, act| {
            env.call_method(act, "pixelsPerMm", "()F", &[])
                .ok()?
                .f()
                .ok()
        })
        .filter(|v| v.is_finite() && *v > 1.0)
    }

    pub fn save_dir() -> Option<std::path::PathBuf> {
        app()?.internal_data_path()
    }

    pub fn read_asset(rel: &str) -> Option<Vec<u8>> {
        use std::io::Read;
        let name = std::ffi::CString::new(rel).ok()?;
        let mut asset = app()?.asset_manager().open(&name)?;
        let mut out = Vec::with_capacity(asset.length());
        asset.read_to_end(&mut out).ok()?;
        Some(out)
    }
}

#[cfg(target_os = "ios")]
mod imp {
    use super::Insets;
    use objc2::encode::{Encode, Encoding};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct UIEdgeInsets {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    }

    unsafe impl Encode for UIEdgeInsets {
        const ENCODING: Encoding = Encoding::Struct("UIEdgeInsets", &[f64::ENCODING; 4]);
    }

    pub fn insets_px() -> Insets {
        unsafe {
            let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
            if app.is_null() {
                return Insets::default();
            }
            let windows: *mut AnyObject = msg_send![app, windows];
            if windows.is_null() {
                return Insets::default();
            }
            let window: *mut AnyObject = msg_send![windows, firstObject];
            if window.is_null() {
                return Insets::default();
            }
            let i: UIEdgeInsets = msg_send![window, safeAreaInsets];
            let screen: *mut AnyObject = msg_send![class!(UIScreen), mainScreen];
            let scale: f64 = msg_send![screen, nativeScale];
            Insets {
                left: (i.left * scale) as f32,
                top: (i.top * scale) as f32,
                right: (i.right * scale) as f32,
                bottom: (i.bottom * scale) as f32,
            }
        }
    }

    pub fn units_per_mm_px() -> Option<f32> {
        unsafe {
            let screen: *mut AnyObject = msg_send![class!(UIScreen), mainScreen];
            if screen.is_null() {
                return None;
            }
            let scale: f64 = msg_send![screen, nativeScale];
            let device: *mut AnyObject = msg_send![class!(UIDevice), currentDevice];
            let idiom: isize = if device.is_null() {
                0
            } else {
                msg_send![device, userInterfaceIdiom]
            };
            let points_per_inch = if idiom == 1 { 132.0 } else { 163.0 };
            Some((scale * points_per_inch / 25.4) as f32)
        }
    }

    pub fn save_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Insets;

    pub fn insets_px() -> Insets {
        let k = crate::web::query("inset")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        if k == 0.0 {
            return Insets::default();
        }
        let w = web_sys::window();
        let (sw, sh) = w
            .map(|w| {
                (
                    w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1.0),
                    w.inner_height()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1.0),
                )
            })
            .unwrap_or((1.0, 1.0));
        if sh > sw {
            Insets {
                top: k,
                bottom: k * 0.35,
                ..Default::default()
            }
        } else {
            Insets {
                left: k,
                bottom: k * 0.35,
                ..Default::default()
            }
        }
    }

    pub fn units_per_mm_px() -> Option<f32> {
        None
    }

    pub fn save_dir() -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
mod imp {
    use super::Insets;

    pub fn insets_px() -> Insets {
        let k = std::env::var("KG_INSET")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        Insets {
            left: k,
            bottom: k * 0.35,
            ..Default::default()
        }
    }

    pub fn units_per_mm_px() -> Option<f32> {
        None
    }

    pub fn save_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("KG_SAVE_DIR").map(std::path::PathBuf::from)
    }
}

pub use imp::{insets_px, save_dir, units_per_mm_px};

pub fn install_asset_reader() {
    #[cfg(target_os = "android")]
    kg_core::assets::set_reader(imp::read_asset);
}

pub fn device() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "Web"
    } else if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(target_os = "ios") {
        "iPhone or iPad"
    } else {
        "Computer"
    }
}
