# Meeting.rs

Meeting.rs is an online one-to-one video meeting application that utilizes WebRTC, Rust, and WASM technologies. It includes a coordinator server and a peer web page, which allow users to have private and real-time video meetings with a single deployment.

## Contents

<details>

- [Features](#features)
- [Demo](#demo)
- [Usage](#usage)
- [Deployment](#deployment)
- [Project status](#project-status)
- [Contribution](#contribution)
- [Contact](#contact)
- [Community](#community)
- [License](#license)
</details>

## Features

- [x] Peer to peer connection through WebRTC, with extremely low latency
- [x] Video and audio communication
- [x] Mute or unmute video and audio
- [x] Screen sharing

## Demo

Please try the demo on https://meeting.shuttleapp.rs.

Note that this demo is hosted on shuttle.rs with a Hobby plan, which means there may be limits on the number of requests.

## Usage

To compile and run the coordinator server, and to compile the peer, you need to have the Rust toolchain installed. You should also install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) and [just](https://github.com/casey/just) to build and run the application.

Once you have these prerequisites installed, run the following command:

```sh
just coordinator
```

Then follow these steps:

1. For each user, open a browser tab and navigate to the following address: http://localhost:3000.
2. Once the page loads, you will find two buttons: "Video and Audio Sharing" and "Screen Sharing". Click on the button corresponding to the content you want to share.
3. The browser will prompt you for permissions. Grant the respective permissions depending on your choice (e.g., camera and microphone access for video and audio sharing, screen sharing permission for screen sharing).
4. After granting the necessary permissions, enter the same passphrase as the other user. This passphrase ensures that both users are connected to the same session.
5. Instantly, both users will be able to see each other in real-time.

For more detailed information, you can access the web console of your browser.

## Deployment

To allow any hosts other than `localhost` to access the coordination server, you can use an Nginx TLS termination proxy. Additionally, the application uses the default Google STUN server, but you can also use your own STUN/TURN server.

## Project status

The Meeting.rs application is currently functioning exceptionally well, and its design emphasizes minimalism and efficiency through the use of Rust. The application is limited to one-to-one meetings, and plans are underway to add additional features.

## Contribution

Contributions to the project are welcome and encouraged!

## Contact

For further information or to discuss your specific requirements, please feel free to reach out to me:

- Email: williamlsh@protonmail.com

## License

GPL-3.0
