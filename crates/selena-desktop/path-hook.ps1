param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("add", "remove")]
    [string]$Action,
    [Parameter(Mandatory = $true)]
    [string]$Target
)

$ErrorActionPreference = "Stop"

# Work with the raw registry value so %USERPROFILE%-style entries are not
# expanded and long PATH values are not truncated by NSIS.
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment", $true)
if ($null -eq $key) {
    throw "Unable to open HKCU\\Environment"
}

try {
    $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
    try {
        $kind = $key.GetValueKind("Path")
    } catch {
        # The value does not exist yet.
    }

    $raw = [string]$key.GetValue(
        "Path",
        "",
        [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    )
    $normalizedTarget = $Target.TrimEnd("\")
    $parts = @($raw -split ";" | Where-Object { $_ -ne "" })

    if ($Action -eq "add") {
        if ($parts -notcontains $normalizedTarget) {
            $parts += $normalizedTarget
        }
    } else {
        $parts = @($parts | Where-Object { $_ -ne $normalizedTarget })
    }

    $key.SetValue("Path", ($parts -join ";"), $kind)
} finally {
    $key.Close()
}
