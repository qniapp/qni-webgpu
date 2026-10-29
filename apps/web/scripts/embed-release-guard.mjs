import { spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

export class EmbedReleaseGuard {
  constructor({ repository, tag, sha, token, runner = args => spawnSync('gh', args, { encoding: 'utf8' }) }) {
    if (!repository || !tag || !/^[a-f0-9]{40}$/.test(sha) || !token) throw new Error('Repository, tag, full SHA and authenticated token are required')
    this.repository = repository
    this.base = `repos/${repository}`
    this.tag = tag
    this.sha = sha
    this.runner = runner
  }

  api(endpoint, { allowAbsent = false } = {}) {
    const result = this.runner(['api', '--include', `${this.base}/${endpoint}`])
    if (result.error) throw result.error
    if (result.status !== 0) {
      // A failed authenticated request proves absence only with an HTTP 404 response.
      if (allowAbsent && /^HTTP\/\S+ 404(?:[^\S\r\n]|\r?$)/m.test(result.stdout ?? '')) return null
      throw new Error(`API failed: ${endpoint}: ${result.stderr ?? ''} ${result.stdout ?? ''}`)
    }
    const body = result.stdout.replace(/^HTTP\/[\s\S]*?\r?\n\r?\n/, '')
    const value = JSON.parse(body)
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`Invalid API response: ${endpoint}`)
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

  releaseIncludingDraft() {
    // REST's release-by-tag endpoint excludes drafts; GraphQL's exact lookup includes them.
    const [owner, name] = this.repository.split('/')
    const query = 'query($owner: String!, $name: String!, $tag: String!) { repository(owner: $owner, name: $name) { release(tagName: $tag) { tagName isDraft } } }'
    const result = this.runner(['api', 'graphql', '-f', `query=${query}`, '-f', `owner=${owner}`, '-f', `name=${name}`, '-f', `tag=${this.tag}`])
    if (result.error) throw result.error
    if (result.status !== 0) throw new Error(`Release lookup failed: ${result.stderr ?? ''} ${result.stdout ?? ''}`)
    const value = JSON.parse(result.stdout)
    const repository = value?.data?.repository
    if (value?.errors || !repository || !Object.hasOwn(repository, 'release')) throw new Error('Invalid release lookup response')
    const release = repository.release
    if (release !== null && (release?.tagName !== this.tag || typeof release?.isDraft !== 'boolean')) throw new Error('Invalid release lookup response')
    return release
  }

  assertAbsent() {
    this.assertTagAbsent()
    if (this.api(`releases/tags/${encodeURIComponent(this.tag)}`, { allowAbsent: true }) !== null) throw new Error('Release already exists')
    if (this.releaseIncludingDraft() !== null) throw new Error('Release already exists (including draft)')
  }

  assertOwnDraft(releaseId) {
    if (!/^[1-9][0-9]*$/.test(releaseId ?? '') || !Number.isSafeInteger(Number(releaseId))) throw new Error('A positive safe integer release id is required')
    const release = this.api(`releases/${releaseId}`)
    if (release.id !== Number(releaseId)) throw new Error('Draft release id differs from recorded id')
    if (release.draft !== true) throw new Error('Expected draft release')
    if (release.tag_name !== this.tag) throw new Error('Draft release tag differs from TAG')
    if (release.target_commitish !== this.sha) throw new Error('Draft release target differs from GITHUB_SHA')
    this.assertTagAbsent()
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
    else throw new Error('Unknown release guard command')
  } catch (error) {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 1
  }
}
