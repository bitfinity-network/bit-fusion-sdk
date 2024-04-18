use jsonrpc_core::{Error, MethodCall, Params as RequestParams};
use serde::de::DeserializeOwned;

pub trait ParamsAccessors {
    /// Get a required parameter from the params array.
    fn get_from_vec<T: DeserializeOwned>(&self, index: usize) -> Result<T, Error>;

    /// Get an optional parameter from the params object.
    fn get_from_object<T: DeserializeOwned>(&self, field: &str) -> Result<Option<T>, Error>;

    /// Checks the type and the number of parameters.
    ///
    /// Fails if
    /// - params is not an array
    /// - number of params is less than `len(req_params)`
    /// - number of params is greater than `max`
    fn validate_params(&self, required_params: &[&str], max_size: usize) -> Result<(), Error>;

    /// Get a required parameter from the params object.
    fn required_from_object<T: DeserializeOwned>(&self, field: &str) -> Result<T, Error> {
        self.get_from_object(field)?
            .ok_or_else(|| Error::invalid_params(format!("missing field '{field}'")))
    }
}

impl ParamsAccessors for MethodCall {
    fn get_from_vec<T: DeserializeOwned>(&self, index: usize) -> Result<T, Error> {
        let RequestParams::Array(params) = &self.params else {
            return Err(Error::invalid_params("missing params"));
        };

        match params.get(index) {
            Some(value) => serde_json::from_value(value.clone()).map_err(|e| {
                Error::invalid_params(format!("failed to deserialize value at index {index}: {e}"))
            }),
            None => Err(Error::invalid_params(format!(
                "index {} exceeds length of params {}",
                index,
                params.len()
            ))),
        }
    }

    fn get_from_object<T: DeserializeOwned>(&self, field: &str) -> Result<Option<T>, Error> {
        let RequestParams::Map(params) = &self.params else {
            return Err(Error::invalid_params(
                "missing params object or params is not an object",
            ));
        };

        match params.get(field) {
            Some(value) if value.is_null() => Ok(None),
            Some(value) => serde_json::from_value(value.clone()).map_err(|e| {
                Error::invalid_params(format!("failed to deserialize value at field {field}: {e}"))
            }),
            None => Ok(None),
        }
    }

    fn validate_params(&self, required_params: &[&str], max_size: usize) -> Result<(), Error> {
        let RequestParams::Array(params) = &self.params else {
            return Err(Error::invalid_params(format!(
                "expected 'params' array of at least {} arguments",
                required_params.len()
            )));
        };

        let param_count = params.len();

        if param_count < required_params.len() {
            return Err(Error::invalid_params(format!(
                "expected at least {} argument/s but received {}: required parameters [{}]",
                required_params.len(),
                param_count,
                required_params.join(", ")
            )));
        }
        if param_count > max_size {
            return Err(Error::invalid_params(format!(
                "too many arguments, want at most {max_size}"
            )));
        }
        Ok(())
    }
}
