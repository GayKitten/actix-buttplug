use actix::{Actor, ActorContext, Addr, Handler, Message, StreamHandler};
use actix_web::{error::PayloadError, web::Bytes, HttpRequest, HttpResponse};
use actix_web_actors::ws::{
	CloseCode, Message as WsMessage, ProtocolError, WebsocketContext, WsResponseBuilder,
};
use buttplug::connector::{transport::ButtplugConnectorTransport, ButtplugConnectorError};
use futures::{future::BoxFuture, Stream};
use tokio::sync::mpsc::{Receiver, Sender};

/// A transport connector similar to [buttplug::ButtplugWebsocketClientTransport]
pub struct ButtplugActixWebsocketTransport {
	inner_addr: Addr<InnerWsTransporterActor>,
}

impl ButtplugActixWebsocketTransport {
	pub fn new<S>(
		req: &HttpRequest,
		stream: S,
	) -> Result<(Self, HttpResponse), actix_web::error::Error>
	where
		S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
	{
		let inner = InnerWsTransporterActor {};
		let (inner_addr, res) = WsResponseBuilder::new(inner, req, stream).start_with_addr()?;

		Ok((Self { inner_addr }, res))
	}
}

impl ButtplugConnectorTransport for ButtplugActixWebsocketTransport {
	fn connect(
		&self,
		outgoing_receiver: Receiver<
			buttplug::core::messages::serializer::ButtplugSerializedMessage,
		>,
		incoming_sender: Sender<buttplug::connector::transport::ButtplugTransportIncomingMessage>,
	) -> BoxFuture<'static, Result<(), buttplug::connector::ButtplugConnectorError>> {
		todo!()
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
struct InnerWsTransporterActor {}

impl Actor for InnerWsTransporterActor {
	type Context = WebsocketContext<Self>;
}

impl StreamHandler<Result<WsMessage, ProtocolError>> for InnerWsTransporterActor {
	fn handle(&mut self, msg: WsMsg, ctx: &mut Self::Context) {
		match msg {
			WsMsg::Text(text) => {
				println!("Got text: {}", text);
			}
			WsMsg::Binary(bin) => {
				println!("Got binary: {:?}", bin);
			}
			WsMsg::Ping(bin) => {
				println!("Got ping: {:?}", bin);
				ctx.pong(&bin);
			}
			WsMsg::Pong(bin) => {
				println!("Got pong: {:?}", bin);
			}
			WsMsg::Close(close) => {
				println!("Got close: {:?}", close);
				ctx.stop();
			}
			_ => {
				println!("Got other message");
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
