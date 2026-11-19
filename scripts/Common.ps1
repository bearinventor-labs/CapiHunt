function Clean-Prompt {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt
    )
    $cleanPrompt = $Prompt.Trim().ToUpper()
    return $cleanPrompt
}