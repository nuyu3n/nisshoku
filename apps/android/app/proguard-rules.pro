# The engine's Kotlin bindings (UniFFI) call into Rust through JNA, which
# finds classes, fields and callbacks by reflection.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class app.nisshoku.android.engine.** { *; }
-dontwarn java.awt.**

# Certificate checks of the engine, called from Rust over JNI.
-keep class org.rustls.platformverifier.** { *; }
