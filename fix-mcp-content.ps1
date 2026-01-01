# Fix MCPContent instantiations by adding service_info: None

$files = @(
    "src\mcp\builtin\playbook\operations.rs",
    "src\mcp\builtin\workspace\ui_resources.rs",
    "src\mcp\builtin\bootstrap\mod.rs"
)

foreach ($file in $files) {
    $fullPath = Join-Path "c:\Users\innoc\my_works\libr-agent\src-tauri" $file
    if (Test-Path $fullPath) {
        Write-Host "Processing $file..."
        $content = Get-Content $fullPath -Raw
        
        # Pattern 1: MCPContent::Text { text: ... }
        $content = $content -replace '(MCPContent::Text\s*\{\s*text:\s*[^}]+)(\s*\})', '$1,${2}service_info: None,$3'
        
        # Pattern 2: MCPContent::Text {\n            text: ...
        $content = $content -replace '(MCPContent::Text\s*\{\s*\r?\n\s*text:\s*[^,]+)(,?\s*\r?\n)', '$1,${2}                service_info: None,$3'
        
        Set-Content $fullPath $content -NoNewline
        Write-Host "Fixed $file"
    }
}

Write-Host "Done!"
