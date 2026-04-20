# Feature: egui web startup success

## Scenario: WebGPU canvas renders content with the standard browser
- Given the egui web app is open in the standard WebGPU browser
- When the app finishes initializing
- Then the WebGPU error is absent
- And the canvas is visible
- And the initial state vector is "[1,0,0,0]"
