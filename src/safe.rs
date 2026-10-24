#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[cfg(target_os = "android")]
pub fn insets_px() -> Insets {
    use macroquad::miniquad::native::android::{attach_jni_env, ndk_sys, ACTIVITY};
    unsafe {
        let env = attach_jni_env();
        let activity = ACTIVITY;
        if activity.is_null() {
            return Insets::default();
        }
        let get_class = (**env).GetObjectClass.unwrap();
        let get_method = (**env).GetMethodID.unwrap();
        let call_object = (**env).CallObjectMethod.unwrap();
        let exception_check = (**env).ExceptionCheck.unwrap();
        let exception_clear = (**env).ExceptionClear.unwrap();
        let get_ints = (**env).GetIntArrayRegion.unwrap();
        let delete_local = (**env).DeleteLocalRef.unwrap();
        let class = get_class(env, activity);
        let method = get_method(env, class, c"safeAreaInsets".as_ptr(), c"()[I".as_ptr());
        if method.is_null() || exception_check(env) != 0 {
            exception_clear(env);
            delete_local(env, class);
            return Insets::default();
        }
        let array = call_object(env, activity, method);
        if array.is_null() || exception_check(env) != 0 {
            exception_clear(env);
            delete_local(env, class);
            return Insets::default();
        }
        let mut out = [0 as ndk_sys::jint; 4];
        get_ints(env, array as ndk_sys::jintArray, 0, 4, out.as_mut_ptr());
        delete_local(env, array);
        delete_local(env, class);
        Insets {
            left: out[0] as f32,
            top: out[1] as f32,
            right: out[2] as f32,
            bottom: out[3] as f32,
        }
    }
}

#[cfg(target_os = "ios")]
pub fn insets_px() -> Insets {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct UIEdgeInsets {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    }

    unsafe impl objc::Encode for UIEdgeInsets {
        fn encode() -> objc::Encoding {
            unsafe { objc::Encoding::from_str("{UIEdgeInsets=dddd}") }
        }
    }

    unsafe {
        let app: *mut Object = msg_send![class!(UIApplication), sharedApplication];
        if app.is_null() {
            return Insets::default();
        }
        let windows: *mut Object = msg_send![app, windows];
        if windows.is_null() {
            return Insets::default();
        }
        let window: *mut Object = msg_send![windows, firstObject];
        if window.is_null() {
            return Insets::default();
        }
        let i: UIEdgeInsets = msg_send![window, safeAreaInsets];
        let screen: *mut Object = msg_send![class!(UIScreen), mainScreen];
        let scale: f64 = msg_send![screen, nativeScale];
        Insets {
            left: (i.left * scale) as f32,
            top: (i.top * scale) as f32,
            right: (i.right * scale) as f32,
            bottom: (i.bottom * scale) as f32,
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn insets_px() -> Insets {
    Insets::default()
}

pub struct SafeArea {
    cached: Insets,
    size: (f32, f32),
    frames: u32,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            cached: Insets::default(),
            size: (0.0, 0.0),
            frames: u32::MAX,
        }
    }
}

impl SafeArea {
    pub fn get(&mut self, screen_px: (f32, f32)) -> Insets {
        if screen_px != self.size {
            self.size = screen_px;
            self.frames = 0;
        }
        if self.frames < 90 && self.frames.is_multiple_of(15) {
            self.cached = insets_px();
        }
        self.frames = self.frames.saturating_add(1);
        self.cached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_has_no_insets() {
        assert_eq!(insets_px(), Insets::default());
    }

    #[test]
    fn rotation_triggers_a_fresh_read() {
        let mut s = SafeArea::default();
        s.get((1000.0, 2000.0));
        assert_eq!(s.frames, 1);
        for _ in 0..200 {
            s.get((1000.0, 2000.0));
        }
        let settled = s.frames;
        s.get((2000.0, 1000.0));
        assert_eq!(s.frames, 1);
        assert!(settled > 90);
    }
}
