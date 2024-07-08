#[macro_export]
macro_rules! build_data {
    () => {{
        // E.g.: x86_64-unknown-linux-gnu
        const CARGO_TARGET_TRIPLE: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");
        // E.g.: default
        const CARGO_FEATURES: &str = env!("VERGEN_CARGO_FEATURES");
        // E.g.: evm
        const PKG_NAME: &str = env!("CARGO_PKG_NAME");
        // E.g.: 0.1.0
        const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
        // E.g.: 1.64.0
        const RUSTC_SEMVER: &str = env!("VERGEN_RUSTC_SEMVER");
        // E.g.: 2022-12-23T15:29:20.000000000Z
        const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
        // E.g.: true/false
        const CARGO_DEBUG: &str = env!("VERGEN_CARGO_DEBUG");
        // E.g.: main
        const GIT_BRANCH: &str = env!("VERGEN_GIT_BRANCH");
        // E.g.: acf6c5744b1f4f29c5960a25f4fb4056e2ceedc3
        const GIT_SHA: &str = env!("VERGEN_GIT_SHA");
        // E.g.: 2022-12-23T15:29:20.000000000Z
        const GIT_COMMIT_TIMESTAMP: &str = env!("VERGEN_GIT_COMMIT_TIMESTAMP");

        ::did::build::BuildData {
            cargo_target_triple: CARGO_TARGET_TRIPLE.to_string(),
            cargo_features: CARGO_FEATURES.to_string(),
            pkg_name: PKG_NAME.to_string(),
            pkg_version: PKG_VERSION.to_string(),
            rustc_semver: RUSTC_SEMVER.to_string(),
            build_timestamp: BUILD_TIMESTAMP.to_string(),
            cargo_debug: CARGO_DEBUG.to_string(),
            git_branch: GIT_BRANCH.to_string(),
            git_sha: GIT_SHA.to_string(),
            git_commit_timestamp: GIT_COMMIT_TIMESTAMP.to_string(),
        }
    }};
}
