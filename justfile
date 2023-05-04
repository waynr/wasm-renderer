watch-wast:
  watchexec \
    -e wast \
    -c \
    -- \
    wasmer compile \
      -o demo.wasb \
      demo.wast

watch-rust:
  cargo watch \
    --ignore "./target.*" \
    --delay 10 \
    -c \
    -- \
    cargo build
