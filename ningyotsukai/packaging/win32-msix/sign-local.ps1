param (
    [Parameter(Mandatory=$true, HelpMessage="Name of the app we're packaging.")]
    [string]$AppName,

    [Parameter(Mandatory=$true, HelpMessage="Name of the architecture we're packaging.")]
    [string]$ArchName,

    [Parameter(Mandatory=$true, HelpMessage="Just put the word 'placebo'")]
    [string]$CertPassword,
)

$ProjectRoot = "$PSScriptRoot\..\..\.."
$CertPath = "$ProjectRoot\cert.pfx"

$WindowsSdkRoot = (Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots").KitsRoot10
$LatestVersion = (Get-ChildItem "$WindowsSdkRoot\bin" | Where-Object { $_.Name -like "10.*" } | Sort-Object Name -Descending | Select-Object -First 1 ).Name
$SdkArch = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x64" }
    "ARM64" { "arm64" }
    default { "x86" }
}

$SdkBins = "$WindowsSdkRoot\bin\$LatestVersion\$SdkArch\"

& "$SdkBins\signtool.exe" sign /fd SHA256 /f $CertPath /p "$CertPassword" "$AppName.$ArchName.msix"