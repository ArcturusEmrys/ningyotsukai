param (
    [Parameter(Mandatory=$true, HelpMessage="Name of the app we're packaging.")]
    [string]$AppName,

    [Parameter(Mandatory=$true, HelpMessage="Name of the binary to package.")]
    [string]$ExeName,

    [Parameter(Mandatory=$true, HelpMessage="Name of the architecture we're packaging.")]
    [string]$ArchName,

    [Parameter(Mandatory=$true, HelpMessage="Where the DLLs are")]
    [string]$VcpkgDllPath,

    [Parameter(Mandatory=$true, HelpMessage="Just put the word 'placebo'")]
    [string]$CertPassword
)

$ProjectRoot = "$PSScriptRoot\..\..\.."
$StagingDirectory = "$ProjectRoot\target\packages\win32-msix\$AppName.$ArchName.appxtmpl"
$TargetDirectory = "$ProjectRoot\target\release"
$AppExe = "$TargetDirectory\$ExeName.exe"
$BrandingPath = "$ProjectRoot\$ExeName\branding"
$PackagingPath = "$ProjectRoot\$ExeName\packaging\win32-msix"
$CertPath = "$ProjectRoot\cert.pfx"

#Construct our staging package.
if (Test-Path $StagingDirectory) {
    Remove-Item $StagingDirectory -Recurse -Force
}

New-Item -ItemType Directory -Path $StagingDirectory

Copy-Item $AppExe -Destination "$StagingDirectory\$ExeName.exe"

New-Item -ItemType Directory -Path "$StagingDirectory\Icon\scale-100"
Copy-Item "$BrandingPath\$AppName Icon 16.png" -Destination "$StagingDirectory\Icon\scale-100\AppList.targetsize-16.png"
Copy-Item "$BrandingPath\$AppName Icon 32.png" -Destination "$StagingDirectory\Icon\scale-100\AppList.targetsize-32.png"
Copy-Item "$BrandingPath\$AppName Icon 64.png" -Destination "$StagingDirectory\Icon\scale-100\AppList.targetsize-64.png"
Copy-Item "$BrandingPath\$AppName Icon 128.png" -Destination "$StagingDirectory\Icon\scale-100\AppList.targetsize-128.png"
Copy-Item "$BrandingPath\$AppName Icon 128.png" -Destination "$StagingDirectory\Icon\scale-100\AppList.png"

New-Item -ItemType Directory -Path "$StagingDirectory\Icon\scale-200"
Copy-Item "$BrandingPath\$AppName Icon 16@2x.png" -Destination "$StagingDirectory\Icon\scale-200\AppList.targetsize-16.png"
Copy-Item "$BrandingPath\$AppName Icon 32@2x.png" -Destination "$StagingDirectory\Icon\scale-200\AppList.targetsize-32.png"
Copy-Item "$BrandingPath\$AppName Icon 64@2x.png" -Destination "$StagingDirectory\Icon\scale-200\AppList.targetsize-64.png"
Copy-Item "$BrandingPath\$AppName Icon 128@2x.png" -Destination "$StagingDirectory\Icon\scale-200\AppList.targetsize-128.png"
Copy-Item "$BrandingPath\$AppName Icon 128@2x.png" -Destination "$StagingDirectory\Icon\scale-200\AppList.png"

Copy-Item "$VcpkgDllPath\*.dll" -Destination "$StagingDirectory\"

Copy-Item "$PackagingPath\AppxManifest.xml" -Destination "$StagingDirectory\AppxManifest.xml"

#Inject the Subject of the certificate we intend to sign with.
$Cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($CertPath, $CertPassword)
[xml]$Manifest = Get-Content "$StagingDirectory\AppxManifest.xml"

if ($Cert) {
    $Manifest.Package.Identity.Publisher = $Cert.Subject;
    $Manifest.Save("$StagingDirectory\AppxManifest.xml")
}

#Find makeappx & assemble the package
$WindowsSdkRoot = (Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots").KitsRoot10
$LatestVersion = (Get-ChildItem "$WindowsSdkRoot\bin" | Where-Object { $_.Name -like "10.*" } | Sort-Object Name -Descending | Select-Object -First 1 ).Name
$SdkArch = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x64" }
    "ARM64" { "arm64" }
    default { "x86" }
}

$SdkBins = "$WindowsSdkRoot\bin\$LatestVersion\$SdkArch\"

& "$SdkBins\makepri.exe" createconfig /cf "$TargetDirectory\priconfig.xml" /dq "Language-en" /pv "10.0" /o
& "$SdkBins\makepri.exe" new /pr $StagingDirectory /cf "$TargetDirectory\priconfig.xml" /of "$StagingDirectory\resources.pri" /o
& "$SdkBins\makeappx.exe" pack /d $StagingDirectory /p "$AppName.$ArchName.msix" /o
& "$SdkBins\signtool.exe" sign /fd SHA256 /f $CertPath /p "$CertPassword" "$AppName.$ArchName.msix"