const test = require('node:test')
const assert = require('node:assert/strict')

const sha = 'a'.repeat(40)
const other = 'b'.repeat(40)
class GuardFixture {
  static async create({ tag, releases = [], error, lookupExists = false, pages, tagBody, tagError, release, calls = [], token = 'mock' } = {}) {
    const { EmbedReleaseGuard } = await import('../apps/web/scripts/embed-release-guard.mjs')
    return new EmbedReleaseGuard({ repository: 'owner/repo', tag: `embed-${sha}`, sha, token, runner(args) {
      calls.push(args)
      const endpoint = args.at(-1)
      if (error) return { status: 1, stdout: error, stderr: 'API failed' }
      if (endpoint.includes('/git/ref/') && tagError) return { status: 1, stdout: tagError }
      if (endpoint.includes('/git/ref/') && tagBody !== undefined) return { status: 0, stdout: JSON.stringify(tagBody) }
      if (args.includes('--paginate')) return { status: 0, stdout: JSON.stringify(pages ?? [[], releases]) }
      if (endpoint.includes('/git/tags/')) return { status: 0, stdout: 'HTTP/2.0 200 OK\r\nContent-Type: application/json\r\n\r\n' + JSON.stringify({ object: { type: 'commit', sha: tag.sha } }) }
      if (endpoint.includes('/git/ref/') && tag) return { status: 0, stdout: 'HTTP/2.0 200 OK\r\nContent-Type: application/json\r\n\r\n' + JSON.stringify({ object: tag.annotated ? { type: 'tag', sha: other } : { type: 'commit', sha: tag.sha } }) }
      if (/\/releases\/[0-9]+$/.test(endpoint) && release !== undefined) return { status: 0, stdout: JSON.stringify(release) }
      if (lookupExists && endpoint.includes('/releases/tags/')) return { status: 0, stdout: '{}' }
      return { status: 1, stdout: 'HTTP/2.0 404 Not Found\r\n\r\n{}' }
    } })
  }
}
const draft = { id: 42, tag_name: `embed-${sha}`, target_commitish: sha, draft: true }
test('absence confirmed by authenticated 404 and all release pages passes', async () => {
  const guard = await GuardFixture.create()
  assert.doesNotThrow(() => guard.assertAbsent())
})
for (const [name, options, message] of [
  ['matching lightweight tag', { tag: { sha } }, /matches GITHUB_SHA: true/],
  ['different lightweight tag', { tag: { sha: other } }, /matches GITHUB_SHA: false/],
  ['matching annotated tag', { tag: { sha, annotated: true } }, /matches GITHUB_SHA: true/],
  ['different annotated tag', { tag: { sha: other, annotated: true } }, /matches GITHUB_SHA: false/],
  ['draft on later page', { releases: [draft] }, /Release already exists/],
  ['published release', { releases: [{ ...draft, draft: false }] }, /Release already exists/],
  ['tag release lookup exists', { lookupExists: true }, /Release already exists/],
  ['authentication failure', { error: 'HTTP/2.0 401 Unauthorized' }, /API failed/],
  ['transport failure', { error: 'network failure' }, /API failed/],
  ['malformed list', { pages: [{}] }, /Invalid releases/],
  ['null tag response', { tagBody: null }, /Invalid API response/],
  ['invalid tag object', { tagBody: {} }, /Invalid tag commit/],
]) {
  test(`absence rejects ${name}`, async () => {
    const guard = await GuardFixture.create(options)
    assert.throws(() => guard.assertAbsent(), message)
  })
}
test('own draft accepts id lookup while the release list is still empty', async () => {
  const guard = await GuardFixture.create({ releases: [], release: draft })
  assert.doesNotThrow(() => guard.assertOwnDraft('42'))
})
test('own draft never discovers identity from the release list', async () => {
  const calls = []
  const guard = await GuardFixture.create({ release: draft, calls })
  guard.assertOwnDraft('42')
  assert.deepEqual(calls.map(args => args.at(-1)), [`repos/owner/repo/releases/42`, `repos/owner/repo/git/ref/tags/embed-${sha}`])
})
for (const [name, options, message, id = '42'] of [
  ['wrong id', { release: draft }, /Draft release id differs/, '43'],
  ['string response id', { release: { ...draft, id: '42' } }, /Draft release id differs/],
  ['tag lookup forbidden', { release: draft, tagError: 'HTTP/2.0 403 Forbidden' }, /API failed/],
  ['published', { release: { ...draft, draft: false } }, /Expected draft release/],
  ['wrong tag', { release: { ...draft, tag_name: 'other' } }, /Draft release tag differs/],
  ['wrong target', { release: { ...draft, target_commitish: other } }, /Draft release target differs/],
  ['missing target', { release: { ...draft, target_commitish: undefined } }, /Draft release target differs/],
  ['missing (404)', {}, /API failed/],
  ['matching tag appeared', { release: draft, tag: { sha } }, /matches GITHUB_SHA: true/],
  ['different tag appeared', { release: draft, tag: { sha: other } }, /matches GITHUB_SHA: false/],
  ['annotated matching tag appeared', { release: draft, tag: { sha, annotated: true } }, /matches GITHUB_SHA: true/],
  ['annotated different tag appeared', { release: draft, tag: { sha: other, annotated: true } }, /matches GITHUB_SHA: false/],
  ['authentication failure', { error: 'HTTP/2.0 403 Forbidden' }, /API failed/],
  ['transport failure', { error: 'network failure' }, /API failed/],
]) {
  test(`own draft rejects ${name}`, async () => {
    const guard = await GuardFixture.create(options)
    assert.throws(() => guard.assertOwnDraft(id), message)
  })
}
for (const id of [undefined, '', '0', '-1', '1.5', '42/../43', '9007199254740992', '042', 'abc']) {
  test(`own draft rejects invalid id ${JSON.stringify(id)}`, async () => {
    const guard = await GuardFixture.create({ release: draft })
    assert.throws(() => guard.assertOwnDraft(id), /A positive safe integer release id is required/)
  })
  test(`own draft makes no API call for invalid id ${JSON.stringify(id)}`, async () => {
    const calls = []
    const guard = await GuardFixture.create({ release: draft, calls })
    try { guard.assertOwnDraft(id) } catch {}
    assert.deepEqual(calls, [])
  })
}
for (const annotated of [false, true]) {
  test(`published tag accepts matching commit (annotated=${annotated})`, async () => {
    const guard = await GuardFixture.create({ tag: { sha, annotated } })
    assert.doesNotThrow(() => guard.assertPublished())
  })
}
for (const [name, options, message] of [['missing', {}, /Published Git tag is missing/], ['wrong commit', { tag: { sha: other } }, /matches GITHUB_SHA: false/], ['transport', { error: 'offline' }, /API failed/]]) {
  test(`published tag rejects ${name}`, async () => {
    const guard = await GuardFixture.create(options)
    assert.throws(() => guard.assertPublished(), message)
  })
}
