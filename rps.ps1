# rps.ps1 - Minimalist CLI helper for terminal saving to RPS (Rust Paste Server)
[CmdletBinding()]
param (
    [Parameter(Position = 0, ValueFromPipeline = $true)]
    [string]$InputObject,

    [Parameter(Position = 1)]
    [string]$FilePath,

    [alias("s")]
    [string]$Server = $env:RPS_SERVER,

    [alias("e")]
    [string]$Ext,

    [alias("p")]
    [string]$Password
)

begin {
    if ([string]::IsNullOrEmpty($Server)) {
        $Server = "http://localhost:8000"
    }
    $content = ""
}

process {
    if ($null -ne $InputObject) {
        $content += $InputObject + "`n"
    }
}

end {
    # If a file path is specified, read the file
    if ($FilePath) {
        if (-not (Test-Path $FilePath)) {
            Write-Error "Error: File '$FilePath' does not exist."
            exit 1
        }
        $content = Get-Content -Raw $FilePath
        if ([string]::IsNullOrEmpty($Ext)) {
            $Ext = [System.IO.Path]::GetExtension($FilePath).TrimStart('.')
        }
    }

    # If no content was piped or read from file, check if stdin is piped
    if ([string]::IsNullOrWhiteSpace($content)) {
        Write-Error "Error: No input provided via file or pipeline."
        Write-Output "Usage: rps.ps1 [[-InputObject] <string>] [[-FilePath] <string>] [-Server <string>] [-Ext <string>] [-Password <string>]"
        Write-Output "       cat file.txt | .\rps.ps1"
        exit 1
    } else {
        # Trim final newline from pipeline builder if it was piped
        if ($null -ne $InputObject) {
            $content = $content.SubString(0, $content.Length - 1)
        }
    }

    # Prepare JSON payload
    $payload = @{ content = $content }
    if ($Password) {
        $payload["password"] = $Password
    }
    $body = $payload | ConvertTo-Json

    try {
        $response = Invoke-RestMethod -Uri "$Server/api/paste" -Method Post -Body $body -ContentType "application/json"
        $pasteId = $response.id
        
        if ($Ext) {
            Write-Output "$Server/$pasteId.$Ext"
        } else {
            Write-Output "$Server/$pasteId"
        }
    } catch {
        $errorMsg = $_.Exception.Message
        if ($null -ne $_.ErrorDetails -and -not [string]::IsNullOrEmpty($_.ErrorDetails.Message)) {
            $errorMsg = $_.ErrorDetails.Message
        } elseif ($null -ne $_.Exception.Response) {
            $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
            $errorMsg = $reader.ReadToEnd()
            $reader.Close()
        }
        Write-Error "Error: Failed to save paste. $errorMsg"
        exit 1
    }
}
