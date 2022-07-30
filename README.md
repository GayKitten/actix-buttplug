# Actix-Buttplug

A library to use the [Buttplug](https://buttplug.io) protocol with the Actix actor model.

Note that this library has no connection or affiliation with the Actix project.

## Quickstart

```rust,no_run
// Create your own actor that'll manage a ButtplugClient.
struct MyButtplugActor {
	// Note that you shouldn't add a ButtplugClient in here, as that will be
	// handled by the actual context of the actor.
	power: f32,
};

impl Actor for MyButtplugActor {
 // Use the ButtplugContext
 // This internally uses a ButtplugClient.
 type Context = ButtplugContext<Self>;

 fn started(&mut self, ctx: &mut Self::Context) {
		// We can use all the usual methods found on ButtplugClient
		// trough the context.
		let fut = ctx.start_scanning();
		ctx.spawn(fut.inito_actor(self));
 }
}

// You will also need to implement a stream handler for buttplug client events.
// If you don't want to add anything special, it's okay to leave it blank.
impl StreamHandler<ButtplugClientEvent> {
	fn handle(&mut self, event: ButtplugClientEvent ctx: &mut Self::Context) {
		// Do something with the event.
		match event {
			ButtplugClientEvent::DeviceAdded(device) => {
				// Do something with the device.
				let fut = device.vibrate(self.power);
				ctx.spawn(fut.into_actor(self));
			}
		}
	}


#[actix_rt::main]
async fn main() {
	// Use a connector for the ButtplugClient.
	let connector = ButtplugInProcessClientConnector::new(None);

	// create and spawn the actor
	ButtplugContext::start_with_connector(
		MyButtplugActor { power: 0.6 },
		"My Client",
		connector
	).await;
}
```
## Features

* `actix-ws-transporter`: Allows you to use an Actix_web websocket server as a connector transport. It is enabled by default.

## Actix websocket transporter

In adition to the ButtplugContext, this library provides a new connector transport
to use Actix Web server websockets endpoints as a way to connect with a Buttplug Server.  

Here's a quick example on how to use:

```rust,no_run
// An actix websocket endpoint
#[get("/buttplug/connect")]
async fn buttplug_connect_endpoint(
	req: HttpRequest,
	stream: web::Payload
) -> Result<HttpResponse> {
	// Create your actor that implements Actor using the ButtplugContext.
	let actor = MyButtplugActor { power: 0.6 };
	// Start it with the request and the websocket stream.
	// This will spawn the actor and give you the address and the http response.
	let (addr, res) = ButtplugContext::start_with_actix_ws_transport(
		actor,
		"My Client",
		req
		stream
	).await?;

	// Return the websocket response.
	res
}
```

## Contributing

If you wish to contribute to the crate, please follow these steps:

1. Fork the repository.
2. Create a new branch for your changes.
3. Create a PR into the development branch.
