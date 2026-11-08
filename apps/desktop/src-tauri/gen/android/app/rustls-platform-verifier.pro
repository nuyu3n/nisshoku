# Called from Rust through JNI only, so R8 cannot see that it is used.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
