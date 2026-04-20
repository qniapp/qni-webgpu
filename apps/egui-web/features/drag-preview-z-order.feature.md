# Feature: egui web drag preview z-order

## Scenario: dragged palette gate stays above the state panel overlay
- Given the egui web app is open in the standard WebGPU browser
- And the app finishes initializing
- When I drag the palette gate from the palette over the state panel
- Then the dragged gate stays above the state panel overlay
