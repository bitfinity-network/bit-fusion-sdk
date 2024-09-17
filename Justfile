import "./scripts/just/build.just"
import "./scripts/just/code_check.just"
import "./scripts/just/fetch_dependencies.just"
import "./scripts/just/test.just"

export RUST_BACKTRACE := "full"
WASM_DIR := env("WASM_DIR", "./.artifact")

# Lists all the available commands
default:
  @just --list


