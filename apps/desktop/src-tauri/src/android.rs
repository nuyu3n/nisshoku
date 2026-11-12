//! Start-up work that only Android needs.

/// Hands the JVM and the activity to `rustls-platform-verifier`, which
/// reqwest uses to check certificates against Android's trust store
/// (including CAs the user installed). Without it every HTTPS request fails.
pub fn init_tls() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    // SAFETY: tao fills the context with the process's JavaVM and the
    // activity before the app is set up, and both outlive the app.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| e.to_string())
}

/// The background sync of `SyncWorker.kt` (WorkManager, with the app closed
/// or in the background): syncs the profile in `dir` without a window.
/// Returns 0 when synced, 1 on failure (retried later), 2 when there was
/// nothing to do (no target, or a sync already running).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_nisshoku_desktop_SyncWorker_nativeSync<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _class: jni::objects::JClass<'caller>,
    context: jni::objects::JObject<'caller>,
    dir: jni::objects::JString<'caller>,
) -> jni::sys::jint {
    let outcome = unowned_env.with_env(|env| -> Result<jni::sys::jint, jni::errors::Error> {
        let dir: String = dir.to_string();
        // Certificates are checked with Android's trust store (as at start).
        if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
            tracing::warn!("could not set up certificate checks: {e}");
        }
        Ok(crate::sync::background_sync(std::path::Path::new(&dir)))
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
