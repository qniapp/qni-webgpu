# Feature: web plain chromium error

## Scenario: default chromium shows a visible WebGPU error instead of a blank page
- Given the web app is open in plain chromium
- When the plain chromium session finishes loading
- Then a visible WebGPU error is shown
