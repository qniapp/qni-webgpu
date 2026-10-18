import { spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

export class EmbedReleaseGuard {
  constructor({ repository, tag, sha, token, runner = args => spawnSync('gh', args, { encoding: 'utf8' }) }) {
    if (!repository || !tag || !/^[a-f0-9]{40}$/.test(sha) || !token) throw new Error('Repository, tag, full SHA and authenticated token are required')
    this.base = `repos/${repository}`
    this.tag = tag
    this.sha = sha
    this.runner = runner
  }

  api(endpoint, { allowAbsent = false, paginate = false } = {}) {
    const result = this.runner(['api', ...(paginate ? ['--paginate', '--slurp'] : ['--include']), `${this.base}/${endpoint}`])
    if (result.error) throw result.error
    if (result.status !== 0) {
      // A failed authenticated request proves absence only with an HTTP 404 response.
      if (allowAbsent && /^HTTP\/\S+ 404(?:[^\S\r\n]|\r?$)/m.test(result.stdout ?? '')) return null
      throw new Error(`API failed: ${endpoint}: ${result.stderr ?? ''} ${result.stdout ?? ''}`)
    }
    const body = paginate ? result.stdout : result.stdout.replace(/^HTTP\/[\s\S]*?\r?\n\r?\n/, '')
    const value = JSON.parse(body)
    if (!value || typeof value !== 'object' || (!paginate && Array.isArray(value))) throw new Error(`Invalid API response: ${endpoint}`)
    return value
  }

  tagCommit() {
    const ref = this.api(`git/ref/tags/${encodeURIComponent(this.tag)}`, { allowAbsent: true })
    if (ref === null) return null
    let object = ref.object
    const seen = new Set()
    while (object?.type === 'tag') {
      if (!/^[a-f0-9]{40}$/.test(object.sha) || seen.has(object.sha)) throw new Error('Invalid annotated tag')
      seen.add(object.sha)
      object = this.api(`git/tags/${object.sha}`).object
    }
    if (object?.type !== 'commit' || !/^[a-f0-9]{40}$/.test(object.sha)) throw new Error('Invalid tag commit')
    return object.sha
  }

  assertTagAbsent() {
    const commit = this.tagCommit()
    if (commit !== null) throw new Error(`Git tag already exists: ${this.tag} -> ${commit}; matches GITHUB_SHA: ${commit === this.sha}`)
  }

  releases() {
    const pages = this.api('releases?per_page=100', { paginate: true })
    if (!Array.isArray(pages) || !pages.length || !pages.every(Array.isArray)) throw new Error('Invalid releases response')
    const releases = pages.flat()
    if (releases.some(release => !release || typeof release.tag_name !== 'string')) throw new Error('Invalid releases entry')
    return releases.filter(release => release.tag_name === this.tag)
  }

  assertAbsent() {
    this.assertTagAbsent()
    if (this.api(`releases/tags/${encodeURIComponent(this.tag)}`, { allowAbsent: true }) !== null) throw new Error('Release already exists')
    if (this.releases().length) throw new Error('Release already exists (including draft)')
  }

  recordDraft() {
    this.assertTagAbsent()
    const releases = this.releases()
    if (releases.length !== 1 || releases[0].draft !== true || !Number.isSafeInteger(releases[0].id) || releases[0].id < 1) throw new Error('Expected exactly one draft release with a valid id')
    return String(releases[0].id)
  }

  assertOwnDraft(releaseId) {
    if (!/^[1-9][0-9]*$/.test(releaseId ?? '')) throw new Error('Recorded release id is required')
    if (this.recordDraft() !== releaseId) throw new Error('Draft release id differs from recorded id')
  }

  assertPublished() {
    const commit = this.tagCommit()
    if (commit === null) throw new Error('Published Git tag is missing')
    if (commit !== this.sha) throw new Error(`Published tag -> ${commit}; matches GITHUB_SHA: false (expected ${this.sha})`)
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const guard = new EmbedReleaseGuard({ repository: process.env.GITHUB_REPOSITORY, tag: process.env.TAG, sha: process.env.GITHUB_SHA, token: process.env.GH_TOKEN })
    const [command, option, releaseId, ...extra] = process.argv.slice(2)
    if (command === 'assert-own-draft' && option === '--release-id' && releaseId && !extra.length) guard.assertOwnDraft(releaseId)
    else if (option !== undefined) throw new Error('Unexpected arguments')
    else if (command === 'assert-absent') guard.assertAbsent()
    else if (command === 'assert-published') guard.assertPublished()
    else if (command === 'record-draft') process.stdout.write(`${guard.recordDraft()}\n`)
    else throw new Error('Unknown release guard command')
  } catch (error) {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 1
  }
}
