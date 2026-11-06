# What the native engine reaches by name: the JNI methods, and the bridge objects whose
# methods it calls back into. A consumer's shrinker keeps these; the rest of the API is
# reached from Kotlin and needs no rule.
-keepclasseswithmembernames class nl.mattix.walkit.Native { native <methods>; }
-keep class nl.mattix.walkit.*Bridge { *; }
