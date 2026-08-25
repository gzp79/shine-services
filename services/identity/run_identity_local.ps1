Set-Location "$PSScriptRoot"
$env:RUST_BACKTRACE = "full"
${env:SHINE--SERVICE--TLS--CERT} = "..\..\certificates\scytta.crt"
${env:SHINE--SERVICE--TLS--KEY} = "..\..\certificates\scytta.key"
${env:SHINE--SERVICE--PORT} = "8443"
cargo run -p shine-identity --release -- test
