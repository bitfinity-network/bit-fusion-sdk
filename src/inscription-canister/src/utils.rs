use bitcoin::script::PushBytesBuf;
use ord_rs::OrdResult;

pub fn to_push_bytes(bytes: &[u8]) -> OrdResult<PushBytesBuf> {
    let mut push_bytes = PushBytesBuf::with_capacity(bytes.len());
    push_bytes.extend_from_slice(bytes)?;
    Ok(push_bytes)
}
