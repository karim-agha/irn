use {
  crate::primitives::{Message, Subscription},
  core::fmt,
  either::Either,
  multihash::Multihash,
  serde_json::Value,
  std::{fmt::Display, str::FromStr},
  thiserror::Error,
};

#[derive(Debug, Error)]
pub enum RequestError {
  InvalidMethod(String),
  MissingField(String),
  Base58Error(#[from] bs58::decode::Error),
  MultihashError(#[from] multihash::Error),
  Deserialization(#[from] serde_json::Error),
}

impl Display for RequestError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{self:?}")
  }
}

pub fn parse_request(
  request: &str,
) -> Result<Either<Message, Subscription>, RequestError> {
  let json = serde_json::Value::from_str(request)?;
  let method = get_string(&json, "method")?;
  match method.as_str() {
    "irn_subscribe" => Ok(Either::Right(parse_subscribe(json)?)),
    "irn_publish" => Ok(Either::Left(parse_publish(json)?)),
    v => Err(RequestError::InvalidMethod(v.to_string())),
  }
}

fn get_string(
  json: &serde_json::Value,
  name: &str,
) -> Result<String, RequestError> {
  if let Some(Value::String(value)) = json.get(name) {
    return Ok(value.clone());
  }
  Err(RequestError::MissingField(name.into()))
}

fn parse_subscribe(
  json: serde_json::Value,
) -> Result<Subscription, RequestError> {
  if let Some(params) = json.get("params") {
    if let Some(topic) = params.get("topic").and_then(|t| t.as_str()) {
      if let Ok(topic) = bs58::decode(topic).into_vec() {
        return Ok(Multihash::from_bytes(&topic)?);
      }
    }
  }
  Err(RequestError::MissingField("params".to_string()))
}

fn parse_publish(json: serde_json::Value) -> Result<Message, RequestError> {
  if let Some(params) = json.get("params") {
    let topic = params.get("topic").and_then(|t| t.as_str()).map(|t| {
      bs58::decode(t)
        .into_vec()
        .map(|topic| Multihash::from_bytes(&topic))
    });

    let content = params
      .get("message")
      .and_then(|t| t.as_str())
      .map(|content| content.as_bytes().to_vec());

    if let Some(content) = content {
      if let Some(Ok(Ok(topic))) = topic {
        return Ok(Message::new(topic, content));
      } else {
        return Err(RequestError::MissingField("params.topic".to_string()));
      }
    } else {
      return Err(RequestError::MissingField("params.message".to_string()));
    }
  }
  Err(RequestError::MissingField("params".to_string()))
}
