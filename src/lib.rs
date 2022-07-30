#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

/// Crate errors.
pub mod error;

/// Connector transport for actix web server websockets.
///
/// This module has a specialized implementation for `ButtplugConnectorTransport`
/// that can be used with actix web websockets.
#[cfg(feature = "actix-ws-transport")]
pub mod transport;

use std::sync::Arc;

use delegate::delegate;
use error::Result;

use actix::{
	dev::{AsyncContextParts, ContextFut, ContextParts, Mailbox},
	Actor, ActorContext, ActorState, Addr, AsyncContext, StreamHandler,
};
use buttplug::{
	client::{ButtplugClient, ButtplugClientDevice, ButtplugClientError, ButtplugClientEvent},
	connector::ButtplugConnector,
	core::messages::{ButtplugCurrentSpecClientMessage, ButtplugCurrentSpecServerMessage},
};

use buttplug::{
	connector::ButtplugRemoteConnector, core::messages::serializer::ButtplugClientJSONSerializer,
};
use transport::ButtplugActixWebsocketTransport;

/// Execution context for buttplug.io actors.
pub struct ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	inner: ContextParts<A>,
	client: ButtplugClient,
}

impl<A> ActorContext for ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	delegate! {
		to self.inner {
			fn stop(&mut self);
			fn terminate(&mut self);
			fn state(&self) -> ActorState;
		}
	}
}

impl<A> AsyncContext<A> for ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	delegate! {
		to self.inner {
			fn address(&self) -> Addr<A>;
			fn spawn<F>(&mut self, fut: F) -> actix::SpawnHandle
			where
				F: actix::ActorFuture<A, Output = ()> + 'static;
			fn wait<F>(&mut self, fut: F)
			where
				F: actix::ActorFuture<A, Output = ()> + 'static;
			fn cancel_future(&mut self, handle: actix::SpawnHandle) -> bool;
		}
	}
	fn waiting(&self) -> bool {
		self.inner.waiting()
			|| self.inner.state() == ActorState::Stopping
			|| self.inner.state() == ActorState::Stopped
	}
}

impl<A> AsyncContextParts<A> for ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	fn parts(&mut self) -> &mut ContextParts<A> {
		&mut self.inner
	}
}

use actix_web::error::PayloadError;
use actix_web::web::Bytes;
use actix_web::HttpResponse;
use futures::{future::BoxFuture, Stream};

impl<A> ButtplugContext<A>
where
	A: Actor<Context = Self>,
	A: StreamHandler<ButtplugClientEvent>,
{
	/// Start a new actor that uses the ButtplugContext with a given connector.
	pub async fn start_with_connector<Con>(act: A, name: &str, connector: Con) -> Result<Addr<A>>
	where
		Con: ButtplugConnector<ButtplugCurrentSpecClientMessage, ButtplugCurrentSpecServerMessage>
			+ 'static,
	{
		let mailbox = Mailbox::default();
		let addr = mailbox.sender_producer();
		let client = ButtplugClient::new(name);
		client.connect(connector).await?;
		let inner = ContextParts::new(addr);
		let mut ctx = ButtplugContext { inner, client };
		ctx.add_stream(ctx.client.event_stream());
		let fut = ContextFut::new(ctx, act, mailbox);
		let addr = fut.address();
		actix_rt::spawn(fut);
		Ok(addr)
	}

	/// Start a buttplug actor using [ButtplugActixWebsocketTransport] as the transport.
	///
	/// # Example
	/// ```rust,no_run
	/// async fn endpoint(
	///     req: HttpRequest,
	///     stream: web::Payload,
	/// ) -> Result<HttpResponse, Error> {
	///     let (addr, res) = ButtplugContext::start_with_actix_ws_transport(
	///         MyOwnActor::default(),
	///         "My Buttplug Actor",
	///         req,
	///         stream,
	///     ).await?;
	///
	///     res
	/// }
	/// ```
	#[cfg(feature = "actix-ws-transport")]
	pub async fn start_with_actix_ws_transport<S>(
		actor: A,
		name: &str,
		req: &actix_web::HttpRequest,
		stream: S,
	) -> Result<(Addr<A>, HttpResponse)>
	where
		A: Actor<Context = ButtplugContext<A>>,
		S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
	{
		let (trans, res) = ButtplugActixWebsocketTransport::new(req, stream)?;

		let conn: ButtplugRemoteConnector<
			ButtplugActixWebsocketTransport,
			ButtplugClientJSONSerializer,
			_,
			_,
		> = ButtplugRemoteConnector::new(trans);

		let addr = Self::start_with_connector(actor, name, conn).await?;

		Ok((addr, res))
	}
}

impl<A> ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	delegate! {
		to self.client {
			/// Return a list of currently connected devices.
			/// This call delegates to [ButtplugClient::devices].
			pub fn devices(&self) -> Vec<Arc<ButtplugClientDevice>>;
			/// Returns the name of the server.
			/// This call delegates to [ButtplugClient::server_name].
			pub fn server_name(&self) -> Option<String>;
			/// Starts scanning for devices.
			/// This call delegates to [ButtplugClient::start_scanning].
			pub fn start_scanning(&self) -> BoxFuture<'static, Result<(), ButtplugClientError>>;
			/// Stops scanning for devices.
			/// This call delegates to [ButtplugClient::stop_scanning].
			pub fn stop_scanning(&self) -> BoxFuture<'static, Result<(), ButtplugClientError>>;
			/// This call delegates to [ButtplugClient::ping].
			pub fn ping(&self) -> BoxFuture<'static, Result<(), ButtplugClientError>>;
		}
	}
}
