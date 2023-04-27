watch-wast:
  watchexec \
    -e wast \
    -c \
    -- \
    wasmer compile \
      -o demo.wasb \
      demo.wast

watch-rust:
  cargo watch -c -- cargo build
