rm -rf dist/
mkdir dist/
EMCC_CFLAGS="--use-port=sdl2 --use-port=sdl2_ttf --use-port=sdl2_image:formats=png --use-preload-plugins -s ASYNCIFY -s ALLOW_MEMORY_GROWTH=1 --embed-file map" cargo build --target wasm32-unknown-emscripten --release
cp src/index.html dist/
cp -r images/ dist/
cp target/wasm32-unknown-emscripten/release/yaw.js dist/
cp target/wasm32-unknown-emscripten/release/yaw.wasm dist/
python3 -m http.server -d dist/
