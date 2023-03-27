peer:
    @wasm-pack build -t web -d ../static/pkg peer

serve: peer
    @python3 -m http.server -d static

coordinator: peer
    @cargo run -r -p coordinator

build: peer
    @cargo build -p coordinator
