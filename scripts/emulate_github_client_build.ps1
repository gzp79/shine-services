$ErrorActionPreference = "Stop"

$profile="release"
$exampleWasmFiles = (Get-Content ./client/examples.json | ConvertFrom-Json).examples
$bindgen=$true
$opt=$false


Write-Host "Build Game"
cargo build --target=wasm32-unknown-unknown -p shine-client --profile ${profile}

Write-Host "Build examples:", $exampleWasmFiles
cargo build --target=wasm32-unknown-unknown -p shine-client --profile ${profile} --examples

Write-Host "Creating latest.json"
$latest = @{ version = "custom"; examples = $exampleWasmFiles }
$latest | ConvertTo-Json -Depth 3 | Set-Content ./dist/latest.json
New-Item -ItemType Directory -Force -Path ./dist/custom | Out-Null

$wasmFiles = @("shine-client") + $exampleWasmFiles
foreach ($wasmFile in $wasmFiles) {
    Write-Host "${wasmFile}.html"
    # replace shine-client to example.js in index.html
    (Get-Content ./client/index.html) -replace "shine-client", "${wasmFile}" | Set-Content ./dist/custom/${wasmFile}.html
}

if ($bindgen) {
    Write-Host "Pack client"
    wasm-bindgen --no-typescript --target web --out-dir ./dist/custom .\target\wasm32-unknown-unknown\${profile}\shine-client.wasm
    foreach ($exampleWasmFile in $exampleWasmFiles) {
        Write-Host "Pack example" $exampleWasmFile
        wasm-bindgen --no-typescript --target web --out-name ${exampleWasmFile} --out-dir ./dist/custom .\target\wasm32-unknown-unknown\${profile}\examples\${exampleWasmFile}.wasm
    }
}

if ($opt) {    
    foreach ($wasmFile in $wasmFiles) {
        Write-Host "Opt $wasmFile"
        wasm-opt -Oz --strip-debug -o ./dist/custom/${wasmFile}_opt.wasm ./dist/custom/${wasmFile}_bg.wasm
        del ./dist/custom/${wasmFile}_bg.wasm
        move ./dist/custom/${wasmFile}_opt.wasm ./dist/custom/${wasmFile}_bg.wasm
    }
}

