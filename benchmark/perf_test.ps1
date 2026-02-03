# Performance Test Script
$iterations = 10

Write-Host "=== Performance Comparison (Average of $iterations runs) ===" -ForegroundColor Cyan

# Test Slime Ultra
$slimeUltraTimes = @()
for ($i = 1; $i -le $iterations; $i++) {
    $time = (Measure-Command { .\compare_slime_ultra.exe | Out-Null }).TotalMilliseconds
    $slimeUltraTimes += $time
}
$slimeUltraAvg = ($slimeUltraTimes | Measure-Object -Average).Average

# Test Slime Standard
$slimeTimes = @()
for ($i = 1; $i -le $iterations; $i++) {
    $time = (Measure-Command { .\compare_slime.exe | Out-Null }).TotalMilliseconds
    $slimeTimes += $time
}
$slimeAvg = ($slimeTimes | Measure-Object -Average).Average

# Test C
$cTimes = @()
for ($i = 1; $i -le $iterations; $i++) {
    $time = (Measure-Command { .\compare_c.exe | Out-Null }).TotalMilliseconds
    $cTimes += $time
}
$cAvg = ($cTimes | Measure-Object -Average).Average

# Display results
Write-Host ""
Write-Host "Results:" -ForegroundColor Green
Write-Host "--------"
Write-Host ("Slime Ultra:    {0:F2} ms" -f $slimeUltraAvg) -ForegroundColor Yellow
Write-Host ("Slime Standard: {0:F2} ms" -f $slimeAvg) -ForegroundColor Yellow
Write-Host ("C (gcc -O3):    {0:F2} ms" -f $cAvg) -ForegroundColor Yellow
Write-Host ""

# Calculate improvement
$improvement = (($slimeAvg - $slimeUltraAvg) / $slimeAvg) * 100
$vsC = (($cAvg - $slimeUltraAvg) / $cAvg) * 100

Write-Host ("Ultra vs Standard: {0:F1}% faster" -f $improvement) -ForegroundColor Cyan
if ($slimeUltraAvg -lt $cAvg) {
    Write-Host ("Ultra vs C: {0:F1}% FASTER 🏆" -f $vsC) -ForegroundColor Green
} else {
    $slower = (($slimeUltraAvg - $cAvg) / $cAvg) * 100
    Write-Host ("Ultra vs C: {0:F1}% slower" -f $slower) -ForegroundColor Red
}
