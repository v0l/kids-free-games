FROM notfl3/cargo-apk
RUN rustup toolchain install stable --profile minimal \
 && rustup default stable \
 && rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
