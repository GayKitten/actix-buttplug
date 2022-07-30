mod error;

#[cfg(feature = "actix-ws-transport")]
pub mod transport;

use error::Result;

use actix::{
	dev::{AsyncContextParts, ContextFut, ContextParts, Mailbox},
	Actor, ActorContext, ActorState, Addr, AsyncContext, StreamHandler,
};
use buttplug::{
	client::{ButtplugClient, ButtplugClientEvent},
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
	fn stop(&mut self) {
		self.inner.stop()
	}

	fn terminate(&mut self) {
		self.inner.terminate()
	}

	fn state(&self) -> actix::ActorState {
		self.inner.state()
	}
}

impl<A> AsyncContext<A> for ButtplugContext<A>
where
	A: Actor<Context = Self>,
{
	fn address(&self) -> actix::Addr<A> {
		self.inner.address()
	}

	fn spawn<F>(&mut self, fut: F) -> actix::SpawnHandle
	where
		F: actix::ActorFuture<A, Output = ()> + 'static,
	{
		self.inner.spawn(fut)
	}

	fn wait<F>(&mut self, fut: F)
	where
		F: actix::ActorFuture<A, Output = ()> + 'static,
	{
		self.inner.wait(fut)
	}

	fn waiting(&self) -> bool {
		self.inner.waiting()
			|| self.inner.state() == ActorState::Stopping
			|| self.inner.state() == ActorState::Stopped
	}

	fn cancel_future(&mut self, handle: actix::SpawnHandle) -> bool {
		self.inner.cancel_future(handle)
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
use futures::Stream;

impl<A> ButtplugContext<A>
where
	A: Actor<Context = Self>,
	A: StreamHandler<ButtplugClientEvent>,
{
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

	/// Start a buttplug actor using the actix websocket transport.
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
