use std::sync::{Arc, Mutex};

use actix_web::{HttpRequest, HttpResponse};
use actix_ws::Message;
use buttplug::core::{
	connector::{
		transport::{ButtplugConnectorTransport, ButtplugTransportIncomingMessage},
		ButtplugConnectorError,
	},
	message::serializer::ButtplugSerializedMessage,
};
use futures::{future::BoxFuture, FutureExt};
use tokio::{
	select,
	sync::{
		mpsc::{Receiver, Sender},
		oneshot, Notify,
	},
};

use log::{error, trace};

/// A transport connector similar to [buttplug::ButtplugWebsocketClientTransport]
pub struct ButtplugActixWebsocketTransport {
	send: Mutex<
		Option<
			oneshot::Sender<(
				Sender<ButtplugTransportIncomingMessage>,
				Receiver<ButtplugSerializedMessage>,
			)>,
		>,
	>,
	notify: Arc<Notify>,
}

impl ButtplugActixWebsocketTransport {
	/// Create a new ButtplugActixWebsocketTransport from a reques.
	/// You most likely want to use [start_with_actix_ws_transport][crate::ButtplugContext::start_with_actix_ws_transport] instead.
	pub fn new(
		req: &HttpRequest,
		stream: actix_web::web::Payload,
	) -> Result<(Self, HttpResponse), actix_web::error::Error> {
		let (res, mut session, mut stream) = actix_ws::handle(req, stream)?;

		let (send, recv) = oneshot::channel();
		let notify = Arc::new(Notify::new());
		let notify_clone = notify.clone();

		let this = Self {
			send: Mutex::new(Some(send)),
			notify,
		};

		tokio::task::spawn_local(async move {
			let notify = notify_clone;
			let (in_tx, mut out_rx) = recv.await.expect("Sender dropped");
			in_tx
				.send(ButtplugTransportIncomingMessage::Connected)
				.await;
			loop {
				select! {
					_ = notify.notified().fuse() => {
						trace!("Notified, closing connection");
						session.close(None).await;
						break;
					}
					out_msg = out_rx.recv() => {
						trace!("Sending message");
						match out_msg {
							Some(ButtplugSerializedMessage::Text(text)) => {
								session.text(text).await;
							}
							Some(ButtplugSerializedMessage::Binary(bin)) => {
								session.binary(bin).await;
							}
							None => {
								trace!("Outgoing channel closed, closing connection");
								session.close(None).await;
								break;
							}
						}
					}
					in_msg = stream.recv() => {
						trace!("Got message");
						let msg = match in_msg {
							None => {
								session.close(None).await;
								break;
							},
							Some(Err(err)) => {
								error!("Websocket protocol error: {err}");
								session.close(Some(actix_ws::CloseCode::Protocol.into())).await;
								break;
							}
							Some(Ok(msg)) => msg,
						};
						let msg: ButtplugTransportIncomingMessage = match msg {
							Message::Text(text) => {
								ButtplugTransportIncomingMessage::Message(ButtplugSerializedMessage::Text(text.into()))
							}
							Message::Binary(bin) => {
								ButtplugTransportIncomingMessage::Message(ButtplugSerializedMessage::Binary(bin.to_vec()))
							}
							Message::Ping(bin) => {
								session.pong(&bin).await;
								continue;
							}
							Message::Pong(_) => {
								continue;
							}
							Message::Close(_) => {
								trace!("Got close message, closing connection");
								ButtplugTransportIncomingMessage::Close("Websocket closed".into())
							}
							Message::Nop => {
								continue;
							}
							Message::Continuation(_) => {
								error!("Got continuation message, closing connection");
								session.close(Some(actix_ws::CloseCode::Protocol.into())).await;
								break;
							}
						};
						in_tx.send(msg).await.expect("Receiver dropped");
					}
				}
			}
		});

		Ok((this, res))
	}
}

impl ButtplugConnectorTransport for ButtplugActixWebsocketTransport {
	fn connect(
		&self,
		outgoing_receiver: Receiver<ButtplugSerializedMessage>,
		incoming_sender: Sender<ButtplugTransportIncomingMessage>,
	) -> BoxFuture<'static, Result<(), ButtplugConnectorError>> {
		let send = self.send.lock().unwrap().take().unwrap();
		Box::pin(async move {
			send.send((incoming_sender, outgoing_receiver))
				.expect("No reason this should fail");
			Ok(())
		})
	}

	fn disconnect(self) -> buttplug::core::connector::ButtplugConnectorResultFuture {
		Box::pin(async move {
			self.notify.notify_one();
			Ok(())
		})
	}
}
