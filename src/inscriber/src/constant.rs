pub(crate) const DUMMY_BITCOIN_PUBKEY: &str =
    "02fcf0210771ec96a9e268783c192c9c0d2991d6e957f319b2aa56503ee15fafdd";
pub(crate) const DUMMY_BITCOIN_ADDRESS: &str = "1Q9ioXoxA7xMCHxsMz8z8MMn99kogyo3FS";

pub const INSCRIBER_METHOD_NAME: &str = "inscribe";

pub const GET_BTC_ADDRESS_METHOD_NAME: &str = "get_bitcoin_address";

pub const GET_INSCRIBER_FEE_METHOD_NAME: &str = "get_inscription_fees";

/// The supported endpoints.
pub const SUPPORTED_ENDPOINTS: &[&str] = &[
    INSCRIBER_METHOD_NAME,
    GET_BTC_ADDRESS_METHOD_NAME,
    GET_INSCRIBER_FEE_METHOD_NAME,
];
