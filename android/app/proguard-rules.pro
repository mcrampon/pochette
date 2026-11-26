# What R8 keeps in the release build, besides what it finds used.

# The core's bindings, which JNA reaches by reflection: the library's functions by their names,
# its structures by their fields, and the callbacks Rust calls back.
-keep class io.github.mcrampon.pochette.core.** { *; }
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
# JNA's desktop parts, which Android doesn't have.
-dontwarn java.awt.**
