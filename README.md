# Meeting.rs

A one to one online video meeting application using WebRTC implemented in Rust and WASM.

This application comprises a coordinator server and a peer web page. Users can enjoy realtime and private video meeting with just one deployment.

## Usage

Suppose you already have Rust toolchain installed, to compile Peer and run Coordinator server, you also need to install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) and [just](https://github.com/casey/just).

```sh
just coordinator
```

Open `http://localhost:3000` in two browser tabs, input the same `passphrase` and assign camera and microphone permissions respectively, two users are then expected to see each other in realtime. Try to repeat several times until connected if it happens to be unsuccessful.

## Deployment

Use a Nginx TLS termination proxy for Coordination server to allow any hosts other than localhost to access.

Besides the default Google STUN server the application uses, you can also use your own STUN/TURN server.

## Project status

This meeting application aims to be minimal and efficient by the power of Rust. It limits to allowing only one to one meeting.

## Contribution

Contributions are most welcome!

## License

GPL-3.0
