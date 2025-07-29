FROM ghcr.io/inkroomtemp/rust_musl_build:1.85.0
RUN set -i "s@mirrors.ustc.edu.cn@mirrors.cernet.edu.cn@g" /etc/apt/sources.list.d/debian.sources && apt update -y && cargo new app

WORKDIR /workdir/app
RUN cargo new derive --lib && cargo new tool && cargo new lib --lib && echo "extern crate proc_macro;" > derive/src/lib.rs
COPY Cargo.toml /workdir/app
COPY lib/Cargo.toml /workdir/app/lib/Cargo.toml
COPY tool/Cargo.toml /workdir/app/tool/Cargo.toml
COPY derive/Cargo.toml /workdir/app/derive/Cargo.toml
RUN cargo build --release -vv --target=$(arch)-unknown-linux-musl --all-features
VOLUME /root/.cargo/git
VOLUME /root/.cargo/registry
COPY . /workdir/app
RUN rm -rf target/$(arch)-unknown-linux-musl/release/deps/iepub-* \
    && rm -rf target/$(arch)-unknown-linux-musl/release/deps/libiepub* \
    && rm -rf target/$(arch)-unknown-linux-musl/release/deps/tool-* \
    && rm -rf target/$(arch)-unknown-linux-musl/release/deps/derive-* \
    && rm -rf target/$(arch)-unknown-linux-musl/release/deps/libderive* \
    && rm -rf target/release/deps/libiepub* \
    && rm -rf target/release/deps/iepub-* \
    && rm -rf target/release/deps/tool-* \
    && rm -rf target/release/deps/derive-* \
    && rm -rf target/release/deps/libderive*
RUN cargo build --release --target=$(arch)-unknown-linux-musl && cp target/$(arch)-unknown-linux-musl/release/tool ./iepub-tool && chmod +x ./tool



FROM scratch
COPY --from=0 /workdir/app/iepub-tool /iepub-tool
CMD ["/iepub-tool"]
