Write-Host "Build"
cargo build --target=wasm32-unknown-unknown -p shine-client --profile release-lto

Write-Host "Pack"
wasm-bindgen --no-typescript --target web --out-name shine-client --out-dir ./dist/custom .\target\wasm32-unknown-unknown\release-lto\shine-client.wasm

Write-Host "Opt"
wasm-opt -Oz --strip-debug -o ./dist/custom/shine-client.wasm ./dist/custom/shine-client_bg.wasm

#del ./dist/custom/shine-client.wasm
#ren ./dist/custom/shine-client_opt.wasm
