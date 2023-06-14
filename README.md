# Meeting.rs

Meeting.rs is an online one-to-one video meeting application that utilizes WebRTC, Rust, and WASM technologies. It includes a coordinator server and a peer web page, which allow users to have private and real-time video meetings with a single deployment.

## Contents

- [Features](#features)
- [Demo](#demo)
- [Usage](#usage)
- [Deployment](#deployment)
- [Project status](#project-status)
- [Contribution](#contribution)
- [Contact](#contact)
- [License](#license)

## Features

- [x] Peer to peer connection through WebRTC, with extremely low latency
- [x] Video and audio communication
- [ ] Mute video or audio
- [ ] screen sharing


## Demo

Please try the demo on https://meeting.shuttleapp.rs.

Please note that this demo is hosted on shuttle.rs with a Hobby plan, which means there may be limits on the number of requests. Additionally, this demo only utilizes a public Google STUN server, so there may be unsuccessful connections in certain scenarios.

## Usage

To compile and run the coordinator server, and to compile the peer, you need to have the Rust toolchain installed. You should also install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) and [just](https://github.com/casey/just) to build and run the application.

Once you have these prerequisites installed, run the following command:

```sh
just coordinator
```

Then, for each user, open http://localhost:3000 in a browser tab, grant camera and microphone permissions, and enter the same passphrase. Both users will be able to see each other in real-time immediately. For more details, you can view the web console of your browser.

## Deployment

To allow any hosts other than `localhost` to access the coordination server, you can use an Nginx TLS termination proxy. Additionally, the application uses the default Google STUN server, but you can also use your own STUN/TURN server.

## Project status

The Meeting.rs application is currently functioning exceptionally well, and its design emphasizes minimalism and efficiency through the use of Rust. The application is limited to one-to-one meetings, and plans are underway to add additional features.

## Contribution

Contributions to the project are welcome and encouraged!

## Contact

For further information or to discuss your specific requirements, please feel free to reach out to me:

* Email: williamlsh@protonmail.com

## License

GPL-3.0
