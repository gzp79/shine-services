$ErrorActionPreference = "Stop"

$profile="release"
$opt=$true

Write-Host "Build"
cargo build --target=wasm32-unknown-unknown -p shine-client --profile ${profile}

Write-Host "Pack"
wasm-bindgen --no-typescript --target web --out-name shine-client --out-dir ./dist/custom .\target\wasm32-unknown-unknown\${profile}\shine-client.wasm

if ($opt) {
    Write-Host "Opt"
    wasm-opt -Oz --strip-debug -o ./dist/custom/shine-client_opt.wasm ./dist/custom/shine-client_bg.wasm
    del ./dist/custom/shine-client_bg.wasm
    copy ./dist/custom/shine-client_opt.wasm ./dist/custom/shine-client_bg.wasm
}

Write-Host "Latest.json"
echo "{ ""version"": ""custom"" }" > ./dist/latest.json

Write-Host "Index.html"
copy ./client/index.html ./dist/custom/