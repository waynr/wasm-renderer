watch:
  watchexec --log-file whatever.log \
    -c \
    -i 'target/**' \
    -e toml,rs \
    'cargo build'
