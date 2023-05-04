peer:
    @wasm-pack build --release --weak-refs --no-pack --no-typescript -t web -d ../static/pkg peer

serve: peer
    @python3 -m http.server -d static

coordinator: peer
    @cargo run -r -p coordinator

build: peer
    @cargo build -r -p coordinator
    @cargo build -p coordinator

image:
    @docker build -t ghcr.io/williamlsh/meeting.rs .
    @docker push ghcr.io/williamlsh/meeting.rs
