use {
    prost::Message,
    serde::{Deserialize, Serialize},
};

/// When serializing to JSON, gives an pair of brackets: `{}`. Useful for use in
/// contract messages when there isn't any intended inputs.
#[derive(Serialize, Deserialize, Message, Clone, Copy, PartialEq, Eq)]
pub struct Empty {}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{from_json, from_proto, to_json, to_proto},
    };

    #[test]
    fn serializing_empty() {
        // json - should serializes to a pair of empty brackets
        assert_eq!(to_json(&Empty {}).unwrap(), b"{}".to_vec().into());
        assert_eq!(from_json::<Empty>(b"{}").unwrap(), Empty {});
        // proto - should serializes to empty bytes
        //
        // TODO: we need to think of whether it's safe to store empty values.
        // the original CosmWasm prevents this.
        assert_eq!(to_proto(&Empty {}), b"".to_vec().into());
        assert_eq!(from_proto::<Empty>(b"").unwrap(), Empty {});
    }
}
