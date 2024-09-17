# Formats the code
[group('build')]
fmt args="":
  cargo fmt --all {{args}}

# Formats the code using the nightly version of rustfmt
[group('build')]
fmt_nightly:
  cargo +nightly fmt --all

# Runs clippy on the code
[group('build')]
clippy args="":
  cargo clippy --all-features --all-targets {{args}}