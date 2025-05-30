$drivers = @(
    @{
        Name = "chrome"
        Url  = "https://storage.googleapis.com/chrome-for-testing-public/137.0.7151.55/win64/chromedriver-win64.zip"
    },
    @{
        Name = "gecko"
        Url  = "https://github.com/mozilla/geckodriver/releases/download/v0.36.0/geckodriver-v0.36.0-win32.zip"
    }
)

$targetDir = ".\target\browser-drivers"
$zipPath = "$targetDir\driver.zip"

function Download-And-ExtractDriver($name, $url) {
    Write-Host "Downloading and extracting $name driver from $url"

    Invoke-WebRequest -Uri $url -OutFile $zipPath
    Expand-Archive -Path $zipPath -DestinationPath $targetDir -Force
    Remove-Item $zipPath
}


# Create target directory if it doesn't exist
if (!(Test-Path -Path $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir | Out-Null
}

foreach ($driver in $drivers) {
    Download-And-ExtractDriver -name $driver.Name -url $driver.Url
}