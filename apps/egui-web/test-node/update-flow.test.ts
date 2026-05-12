const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const updateFlowPath = path.join(rootDir, 'src', 'app', 'update_flow.rs')
const viewportPath = path.join(rootDir, 'src', 'app', 'state_panel', 'viewport.rs')

test('state panel overlay uses a layout refreshed after interactions', async () => {
  const source = await fs.readFile(updateFlowPath, 'utf8')

  const initialLayout = source.indexOf(
    'let mut state_frame = self.prepare_state_panel_frame(screen_rect);'
  )
  const interactions = source.indexOf('self.process_state_panel_interactions', initialLayout)
  const refreshedLayout = source.indexOf(
    'state_frame = self.prepare_state_panel_frame(screen_rect);',
    interactions
  )
  const overlayDraw = source.indexOf('self.draw_frame_overlay', refreshedLayout)

  assert.notEqual(initialLayout, -1, 'expected an initial state-panel layout before hit testing')
  assert.notEqual(interactions, -1, 'expected interactions to run after the initial layout')
  assert.notEqual(
    refreshedLayout,
    -1,
    'expected layout refresh after zoom/aspect/resize interactions'
  )
  assert.notEqual(overlayDraw, -1, 'expected overlay draw to use the refreshed layout')
})

test('state panel wheel zoom anchors against the zoomed layout origin', async () => {
  const source = await fs.readFile(viewportPath, 'utf8')

  assert.match(source, /let desired_origin = anchor - from_origin \* scale;/)
  assert.match(source, /QniApp::grid_offset_for_origin\(/)
  assert.doesNotMatch(source, /grid_offset\s*-=/)
})

export {}
