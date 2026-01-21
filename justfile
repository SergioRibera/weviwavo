default:
    cargo run -F desktop
    # dx serve --hot-patch --features desktop

genkey key_alias key_store:
    # Run keytool to generate key
    docker run --rm -it -v "$(pwd)/:/src" -w /src --entrypoint keytool rust-android:1.90-sdk-36 \
        -genkey -v -keystore {{key_store}} -alias {{key_alias}} -keyalg RSA -keysize 2048 -validity 10000

build:
    docker run --rm -it -v "$(pwd)/:/src" -v "$(pwd)/.gradle/caches:/root/.gradle/caches" -w /src rust-android:1.90-sdk-36 assembleRelease

shell:
    docker run --rm -it -v "$(pwd)/:/src" -v "$(pwd)/.gradle/caches:/root/.gradle/caches" -w /src --entrypoint /bin/bash rust-android:1.90-sdk-36

sign key_alias key_store:
    # Run apksigner to sign generated apk
    docker run --rm -it -v "$(pwd)/:/src" -w /src --entrypoint apksigner rust-android:1.90-sdk-36 \
        sign --ks-key-alias {{key_alias}} --ks {{key_store}} android/build/outputs/apk/release/android-release-unsigned.apk
    sudo cp android/build/outputs/apk/release/android-release-unsigned.apk \
        android/build/outputs/apk/release/android-release-signed.apk

install:
    adb install android/build/outputs/apk/release/android-release-signed.apk

run key_alias key_store: (build) (sign key_alias key_store) (install)
