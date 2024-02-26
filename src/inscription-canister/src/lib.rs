#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

// Enable Candid export
ic_cdk::export_candid!();
