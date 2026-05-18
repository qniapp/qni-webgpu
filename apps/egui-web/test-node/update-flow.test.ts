const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const updateFlowPath = path.join(rootDir, 'src', 'app', 'update_flow.rs')
const viewportPath = path.join(rootDir, 'src', 'app', 'state_panel', 'viewport.rs')
const gpuPlanStatePath = path.join(rootDir, 'src', 'app', 'gpu_plan_state.rs')

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

  assert.deepEqual({
    hasInitialLayout: initialLayout !== -1,
    hasInteractionsAfterInitialLayout: interactions !== -1,
    hasRefreshedLayoutAfterInteractions: refreshedLayout !== -1,
    hasOverlayDrawAfterRefresh: overlayDraw !== -1,
  }, {
    hasInitialLayout: true,
    hasInteractionsAfterInitialLayout: true,
    hasRefreshedLayoutAfterInteractions: true,
    hasOverlayDrawAfterRefresh: true,
  })
})

test('circuit step preview hover keeps the cached GPU plan clean', async () => {
  const source = await fs.readFile(gpuPlanStatePath, 'utf8')
  const body = source.match(/pub\(crate\) fn mark_step_preview_dirty\(&mut self\) \{([\s\S]*?)\n    \}/)?.[1] ?? ''

  assert.deepEqual({
    hasCachedPlanComment: body.includes('must not dirty the simulation plan'),
    avoidsRecomputeDirtyFlag: !/needs_recompute\s*=\s*true/.test(body),
  }, { hasCachedPlanComment: true, avoidsRecomputeDirtyFlag: true })
})

test('state panel wheel zoom anchors against the zoomed layout origin', async () => {
  const source = await fs.readFile(viewportPath, 'utf8')

  assert.deepEqual({
    usesZoomedDesiredOrigin: /let desired_origin = anchor - from_origin \* scale;/.test(source),
    usesGridOffsetForOrigin: /QniApp::grid_offset_for_origin\(/.test(source),
    avoidsDeltaGridOffset: !/grid_offset\s*-=/.test(source),
  }, { usesZoomedDesiredOrigin: true, usesGridOffsetForOrigin: true, avoidsDeltaGridOffset: true })
})

export {}
