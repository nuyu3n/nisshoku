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
