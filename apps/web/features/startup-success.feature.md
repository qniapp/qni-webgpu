# Feature: web startup success

## Scenario: WebGPU error is absent with the standard browser
- Given the web app is open in the standard WebGPU browser
- When the app finishes initializing
- Then the WebGPU error is absent

## Scenario: WebGPU canvas becomes visible with the standard browser
- Given the web app is open in the standard WebGPU browser
- When the app finishes initializing
- Then the canvas is visible

## Scenario: WebGPU canvas renders content with the standard browser
- Given the web app is open in the standard WebGPU browser
- When the app finishes initializing
- Then the canvas renders non-background content

## Scenario: initial state vector is available with the standard browser
- Given the web app is open in the standard WebGPU browser
- When the app finishes initializing
- Then the initial state vector is "[1,0,0,0]"
