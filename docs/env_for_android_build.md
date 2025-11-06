# Setting up android build env

## Required tools

- rust toolchains
  - `rustup target add aarch64-linux-android armv7-linux-androideabi`
- Android Studio
  - Simpler to manage tools, jdk, sdk, ndk etc.
  - Install packages: 
    - SDK platform
        - Android 11
    - SDK tools
      - CMake, NDK, Android SDK Command-line Tools
  - add to path: `C:\Program Files\Android\Android Studio\jbr`

## Some useful commands

- `.\gradlew dependencyUpdates` - list outdated packages for lib.version.toml, run in the client folder
- `./gradlew assembleRelease -Pflavor=curve` - build a flavor (example)



## Links

- <https://blog.erikhorton.com/2024/03/31/deploy-bevy-to-android-and-wasm.html>
- <https://blog.erikhorton.com/2025/02/15/bevy-and-android.html>