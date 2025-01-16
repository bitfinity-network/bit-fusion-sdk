BUNDLE_DIR:="./target/bundles"
BRIDGE_DEPLOYER_DIR:=BUNDLE_DIR + "/bridge-deployer"

# Bundle the Bridge-deployer tool
[group('bundle')]
bundle_bridge_deployer: build_solidity build_all_canisters build_bridge_tool
  mkdir -p "{{BRIDGE_DEPLOYER_DIR}}"
  mkdir -p "{{BRIDGE_DEPLOYER_DIR}}/.artifact"

  cp target/release/bridge-deployer "{{BRIDGE_DEPLOYER_DIR}}"
  cp {{WASM_DIR}}/*.wasm.gz "{{BRIDGE_DEPLOYER_DIR}}/.artifact"
  cp -r solidity/ "{{BRIDGE_DEPLOYER_DIR}}"

  tar -czf "target/release/bridge-deployer.tar.gz" "{{BRIDGE_DEPLOYER_DIR}}"

  echo "Bundle placed in target/release/bridge-deployer.tar.gz"

# Bundle all
[group('bundle')]
bundle_all: bundle_bridge_deployer
