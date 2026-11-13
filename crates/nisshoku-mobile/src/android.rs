//! What the app sets up before opening the engine, over JNI (UniFFI has no
//! JNI environment to hand): certificate checks, and the log.

use std::ffi::{CString, c_char, c_int};

/// Hands the JVM and the app's context to `rustls-platform-verifier`, which
/// reqwest uses to check certificates against Android's trust store
/// (including CAs the user installed). Without it every HTTPS request fails.
/// Also sends the engine's log to logcat (tag `nisshoku`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_nisshoku_android_engine_NativeInit_init<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _class: jni::objects::JClass<'caller>,
    context: jni::objects::JObject<'caller>,
) {
    init_log();
    let outcome = unowned_env.with_env(|env| -> Result<(), jni::errors::Error> {
        if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
            tracing::warn!("could not set up certificate checks: {e}");
        }
        Ok(())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// Lines of the log, each written to logcat.
struct Logcat;

impl std::io::Write for Logcat {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let text = text.trim_end();
        if !text.is_empty()
            && let Ok(text) = CString::new(text.replace('\0', " "))
        {
            // 4: ANDROID_LOG_INFO.
            unsafe { __android_log_write(4, c"nisshoku".as_ptr(), text.as_ptr()) };
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn init_log() {
    let _ = tracing_subscriber::fmt()
        .with_writer(|| Logcat)
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .try_init();
}
