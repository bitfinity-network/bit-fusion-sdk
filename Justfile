import "./just/build.just"
import "./just/code_check.just"
import "./just/dfx.just"
import "./just/docker.just"
import "./just/fetch_dependencies.just"
import "./just/test.just"
import "./just/deploy.just"

export RUST_BACKTRACE := "full"
WASM_DIR := env("WASM_DIR", "./.artifact")

# Lists all the available commands
default:
  @just --list
