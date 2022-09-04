use {
  serde::{de::DeserializeOwned, Serialize},
  serde_json::Value,
};

enum JsonRpcVersion {
  V1,
  V2,
}

struct Request<T: DeserializeOwned> {
  id: String,
  method: String,
  version: Option<JsonRpcVersion>,
  params: T,
}

struct ResponseError {
  pub code: i32,
  pub message: String,
  pub data: Option<Value>,
}

enum ResponseOutput<T: Serialize> {
  Result(T),
  Error(ResponseError),
}

struct Response<T: Serialize> {
  id: String,
  version: Option<JsonRpcVersion>,
  output: ResponseOutput<T>,
}

impl<T: Serialize> Response<T> {
  pub fn success<R: DeserializeOwned>(req: Request<R>, value: T) -> Self {
    Self::from_request(req, ResponseOutput::Result(value))
  }

  pub fn failure<R: DeserializeOwned>(
    req: Request<R>,
    value: ResponseError,
  ) -> Self {
    Self::from_request(req, ResponseOutput::<T>::Error(value))
  }

  fn from_request<R: DeserializeOwned>(
    req: Request<R>,
    output: ResponseOutput<T>,
  ) -> Self {
    Self {
      id: req.id,
      version: req.version,
      output,
    }
  }
}
