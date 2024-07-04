enum UserOperations {
    PredefinedSteps,

    // Custom
    IcrcBurn,
    IcrcMint,
}

// in the lib
enum PredefinedSteps {
    OrderSign,
    OrderSinged,
    OrderSent,
    WrappedBurnt,
}

// in the lib
trait Operation {
    async fn progress(self) -> Result<Self, Error>;
}

impl Operation for UserOperations {
    async fn progress(self) {
        match self {
            // predefine
            UserOperations::PredefinedSteps => step.progress(),

            // UserOperations::MintOrderSigning => bridge_lib::sign_mint_order(),
            // UserOperations::MintOrderSending => bridge_lib::send_mint_order(),
            // UserOperations::MintOrderSent => bridge_lib::remove_mint_order(),

            // custom
            UserOperations::IcrcBurn => todo!(),
            UserOperations::IcrcMint => todo!(),
        }
    }
}
