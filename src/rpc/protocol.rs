use {
  crate::primitives::{Message, Subscription},
  either::Either,
  serde_json::Value,
  thiserror::Error,
};

#[derive(Debug, Error)]
pub enum RequestError {}

pub fn parse_request(
  _request: &str,
) -> Result<Either<Message, Subscription>, RequestError> {
  todo!();
}
