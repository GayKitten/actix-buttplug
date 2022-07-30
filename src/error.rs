use buttplug::client::ButtplugClientError;
use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Library errors
#[derive(Debug, Error)]
pub enum Error {
	#[error("Buttplug client error: {0}")]
	ClientError(#[from] ButtplugClientError),
	#[error("Actix web error: {0}")]
	ActixWeb(#[from] actix_web::Error),
}
