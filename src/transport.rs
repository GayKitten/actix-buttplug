use actix::{Actor, ActorContext, Addr, Handler, Message, StreamHandler};
use actix_web::{error::PayloadError, web::Bytes, HttpRequest, HttpResponse};
use actix_web_actors::ws::{
	CloseCode, Message as WsMessage, ProtocolError, WebsocketContext, WsResponseBuilder,
};
use buttplug::{
	connector::{
		transport::{ButtplugConnectorTransport, ButtplugTransportIncomingMessage},
		ButtplugConnectorError,
	},
	core::messages::serializer::ButtplugSerializedMessage,
};
use futures::{future::BoxFuture, Stream};
use tokio::sync::mpsc::{Receiver, Sender};

use log::{error, trace};

/// A transport connector similar to [buttplug::ButtplugWebsocketClientTransport]
pub struct ButtplugActixWebsocketTransport {
	inner_addr: Addr<InnerWsTransporterActor>,
}

impl ButtplugActixWebsocketTransport {
	/// Create a new ButtplugActixWebsocketTransport from a reques.
	/// You most likely want to use [start_with_actix_ws_transport][crate::ButtplugContext::start_with_actix_ws_transport] instead.
	pub fn new<S>(
		req: &HttpRequest,
		stream: S,
	) -> Result<(Self, HttpResponse), actix_web::error::Error>
	where
		S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
	{
		let inner = InnerWsTransporterActor { in_tx: None };
		let (inner_addr, res) = WsResponseBuilder::new(inner, req, stream).start_with_addr()?;

		Ok((Self { inner_addr }, res))
	}
}

impl ButtplugConnectorTransport for ButtplugActixWebsocketTransport {
	fn connect(
		&self,
		mut outgoing_receiver: Receiver<ButtplugSerializedMessage>,
		incoming_sender: Sender<ButtplugTransportIncomingMessage>,
	) -> BoxFuture<'static, Result<(), ButtplugConnectorError>> {
		self.inner_addr.do_send(SendTx(incoming_sender));
		let inner_addr = self.inner_addr.clone();
		Box::pin(async move {
			loop {
				if let Some(out_msg) = outgoing_receiver.recv().await {
					inner_addr
						.send(SendOut(out_msg))
						.await
						.expect("Failed to send outgoing message");
				} else {
					inner_addr
						.send(Disconnect)
						.await
						.expect("Failed to send disconnect message");
				}
			}
		})
	}

	fn disconnect(self) -> buttplug::connector::ButtplugConnectorResultFuture {
		let ret: buttplug::connector::ButtplugConnectorResultFuture = Box::pin(async move {
			self.inner_addr.do_send(Disconnect);
			Ok(())
		});

		ret
	}
}

/// The inner implementation of the transport.
/// This is the actor that handles the websocket connection.
struct InnerWsTransporterActor {
	in_tx: Option<Sender<ButtplugTransportIncomingMessage>>,
}

impl Actor for InnerWsTransporterActor {
	type Context = WebsocketContext<Self>;
}

impl StreamHandler<Result<WsMessage, ProtocolError>> for InnerWsTransporterActor {
	fn handle(&mut self, incoming: Result<WsMessage, ProtocolError>, ctx: &mut Self::Context) {
		// always handle pings.
		if let Ok(WsMessage::Ping(bin)) = &incoming {
			trace!("Got ping: {:?}", bin);
			ctx.pong(bin);
			return;
		}

		if let Err(e) = incoming {
			error!("Got error: {:?}", e);
			ctx.stop();
			return;
		}

		if let Some(in_tx) = &self.in_tx {
			match incoming {
				Ok(WsMessage::Text(text)) => {
					trace!("Received text: {}", text);
					let msg = ButtplugSerializedMessage::Text(text.to_string());
					let future = async {
						in_tx
							.send(ButtplugTransportIncomingMessage::Message(msg))
							.await
							.expect("Failed to send incoming message");
					};
					tokio_scoped::scope(|scope| {
						scope.spawn(future);
					});
				}
				Ok(WsMessage::Binary(bin)) => {
					trace!("Received binary data: {:?}", bin);
					let msg = ButtplugSerializedMessage::Binary(bin.to_vec());
					let fut = async {
						in_tx
							.send(ButtplugTransportIncomingMessage::Message(msg))
							.await
							.expect("Failed to send incoming message");
					};
					tokio_scoped::scope(|scope| {
						scope.spawn(fut);
					});
				}
				_ => (),
			}
		}
	}
}

#[derive(Message)]
#[rtype(result = "()")]
#[rtype(result = "()")]
struct SendTx(Sender<ButtplugTransportIncomingMessage>);

impl Handler<SendTx> for InnerWsTransporterActor {
	type Result = ();

	fn handle(&mut self, msg: SendTx, _ctx: &mut Self::Context) -> Self::Result {
		self.in_tx = Some(msg.0);
	}
}

#[derive(Message)]
#[rtype(result = "()")]
struct SendOut(ButtplugSerializedMessage);

impl Handler<SendOut> for InnerWsTransporterActor {
	type Result = ();

	fn handle(&mut self, SendOut(msg): SendOut, ctx: &mut Self::Context) -> Self::Result {
		match msg {
			ButtplugSerializedMessage::Text(text) => {
				trace!("Sending text: {}", text);
				ctx.text(text);
			}
			ButtplugSerializedMessage::Binary(bin) => {
				trace!("Sending binary data: {:?}", bin);
				ctx.binary(bin);
			}
		}
	}
}

#[derive(Message)]
#[rtype("()")]
struct Disconnect;

impl Handler<Disconnect> for InnerWsTransporterActor {
	type Result = ();

	fn handle(&mut self, _msg: Disconnect, ctx: &mut Self::Context) {
		ctx.close(Some(CloseCode::Normal.into()));
		ctx.stop();
	}
}
