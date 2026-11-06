param(
  [Parameter(Mandatory = $true)][string]$Releases,
  [Parameter(Mandatory = $true)][string]$Old,
  [Parameter(Mandatory = $true)][string]$New
)
$ErrorActionPreference = "Stop"

function Say($text) { Write-Host ":: $text" }
function Fail($text) { Write-Host ":: FAIL $text"; exit 1 }

$Releases = (Resolve-Path $Releases).Path
$work = Join-Path $env:RUNNER_TEMP "blackline-update-check"
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Path $work | Out-Null
$apps = Join-Path $work "Apps"
$exe = Join-Path $apps "blackline.exe"
$data = Join-Path $work "Blackline"
New-Item -ItemType Directory -Path $apps, (Join-Path $data "scripts") | Out-Null
Set-Content -Path (Join-Path $data "scripts\keep-me.fountain") -Value "INT. KITCHEN - NIGHT"

function Version($path) { (& $path --version | Out-String).Trim() }

function Url($dir) { "file:///" + ($dir -replace '\\', '/') }

function Attempt($source) {
  $updater = Join-Path $work "blackline-updater.exe"
  Copy-Item $exe $updater -Force
  & $updater --ncf-update $New --parent 0 --target $exe --data $data --source (Url $source) --name Blackline --slug blackline --repo NativeCompute/blackline --headless --no-reopen | Out-Host
  return $LASTEXITCODE
}

function Pack($from, $to, $version, [string]$exePath) {
  $name = "blackline-v$version-windows-x86_64"
  $stage = Join-Path $work "stage-$version-$(Split-Path $to -Leaf)\$name"
  New-Item -ItemType Directory -Path $stage -Force | Out-Null
  Copy-Item $exePath (Join-Path $stage "blackline.exe")
  $zip = Join-Path $to "$name.zip"
  Compress-Archive -Path $stage -DestinationPath $zip -Force
  $hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
  "$hash  $name.zip" | Out-File -Encoding ascii -NoNewline "$zip.sha256"
}

Say "1. Blackline $Old in a folder of its own"
$oldZip = Join-Path $Releases "blackline-v$Old-windows-x86_64.zip"
Expand-Archive -LiteralPath $oldZip -DestinationPath (Join-Path $work "old")
Copy-Item (Get-ChildItem -Recurse (Join-Path $work "old") -Filter blackline.exe | Select-Object -First 1).FullName $exe
if ((Version $exe) -ne "blackline $Old") { Fail "the installed version is not $Old" }

Say "2. A damaged download changes nothing"
$bad = Join-Path $work "bad-sum"
New-Item -ItemType Directory -Path $bad | Out-Null
Copy-Item (Join-Path $Releases "blackline-v$New-windows-x86_64.zip") $bad
("0" * 64 + "  x") | Out-File -Encoding ascii -NoNewline (Join-Path $bad "blackline-v$New-windows-x86_64.zip.sha256")
if ((Attempt $bad) -eq 0) { Fail "a damaged download was accepted" }
if ((Version $exe) -ne "blackline $Old") { Fail "a damaged download changed the app" }

Say "3. A missing download changes nothing"
$empty = Join-Path $work "empty"
New-Item -ItemType Directory -Path $empty | Out-Null
if ((Attempt $empty) -eq 0) { Fail "a missing download was accepted" }
if ((Version $exe) -ne "blackline $Old") { Fail "a missing download changed the app" }

Say "4. A new version that won't start is never installed"
$broken = Join-Path $work "broken"
New-Item -ItemType Directory -Path $broken | Out-Null
$junk = Join-Path $work "junk.exe"
Set-Content -Path $junk -Value "not a program"
Pack $null $broken $New $junk
if ((Attempt $broken) -eq 0) { Fail "a broken new version was accepted" }
if ((Version $exe) -ne "blackline $Old") { Fail "a broken new version changed the app" }

Say "5. The update itself"
if ((Attempt $Releases) -ne 0) { Get-Content (Join-Path $data "update.log") | Out-Host; Fail "the update did not finish" }
Get-Content (Join-Path $data "update.log") | Out-Host
if ((Version $exe) -ne "blackline $New") { Fail "the app was not updated to $New" }
if (-not (Select-String -Path (Join-Path $data "update.log") -Pattern "Updated to $New" -Quiet)) { Fail "the log does not say the update finished" }
if (-not (Get-ChildItem $work -Directory -Filter "Blackline-backup-*")) { Fail "the library was not copied aside first" }
if (-not (Test-Path (Join-Path $data "scripts\keep-me.fountain"))) { Fail "a script went missing" }
if (-not (Test-Path (Join-Path $apps "blackline.old.exe"))) { Fail "the old version was not kept aside until the next start" }

Say "PASS: $Old updated itself to $New, and every failure left $Old in place"
