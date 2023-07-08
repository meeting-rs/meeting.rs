build_css:
    @npx tailwindcss -i ./static/input.css -o ./static/output.css

peer: build_css
    @wasm-pack build --release --no-typescript -t web -d ../static/pkg peer

serve: peer
    @python3 -m http.server -d static

coordinator: peer
    @cargo run -r -p coordinator

build: peer
    @cargo build -r -p coordinator
