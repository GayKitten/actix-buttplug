#[cfg(feature = "actix-ws-transport")]
mod transport;

use actix::{
	dev::{AsyncContextParts, ContextParts},
	Actor, ActorContext, ActorState, Addr, AsyncContext,
};
use buttplug::{
	client::ButtplugClient, connector::ButtplugConnector, core::messages::ButtplugMessage,
};

/// Execution context for buttplug.io actors.
struct ButtplugContext<A>
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
{
	pub fn start_with_connector<I, O>(
		actor: A,
		name: &str,
		conn: impl ButtplugConnector<I, O>,
	) -> Addr<A>
	where
		A: Actor<Context = ButtplugContext<A>>,
		I: ButtplugMessage + 'static,
		O: ButtplugMessage + 'static,
	{
		let client = ButtplugClient::new(name);
		// TODO: Finish this
		todo!()
	}

	/// Start a buttplug actor using the actix websocket transport.
	#[cfg(feature = "actix-ws-transport")]
	pub fn start_with_actix_ws_transport<S>(
		actor: A,
		name: &str,
		req: &actix_web::HttpRequest,
		stream: S,
	) -> Result<(Addr<A>, HttpResponse), actix_web::error::Error>
	where
		A: Actor<Context = ButtplugContext<A>>,
		S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
	{
		use buttplug::{
			connector::ButtplugRemoteConnector,
			core::messages::serializer::ButtplugClientJSONSerializer,
		};
		use transport::ButtplugActixWebsocketTransport;

		let (trans, res) = ButtplugActixWebsocketTransport::new(req, stream)?;

		let conn: ButtplugRemoteConnector<
			ButtplugActixWebsocketTransport,
			ButtplugClientJSONSerializer,
			_,
			_,
		> = ButtplugRemoteConnector::new(trans);

		let addr = Self::start_with_connector(actor, name, conn);

		Ok((addr, res))
	}
}
