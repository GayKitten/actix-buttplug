use buttplug::client::ButtplugClientError;
use thiserror::Error;

/// A convenience type for results returned by the library.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Library errors
#[derive(Debug, Error)]
pub enum Error {
	/// An error that occurred with ButtplugClients
	#[error("Buttplug client error: {0}")]
	ClientError(#[from] ButtplugClientError),
	/// An error that occurred with Actix web
	#[error("Actix web error: {0}")]
	ActixWeb(#[from] actix_web::Error),
}
