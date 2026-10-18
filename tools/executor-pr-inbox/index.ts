import {
  decodeJson,
  defineApp,
  defineProvider,
  number,
  object,
  oauth2,
  query,
  router,
  secrets,
  string,
  type AccountOf,
  type QueryContext,
  type Sql,
  type SqlReader,
  type Webhook,
  type WebhookContext,
} from "apps";

const User = object({
  id: number(),
  login: string(),
  avatar_url: string(),
  html_url: string(),
});

const github = defineProvider({
  name: "GitHub",
  hosts: ["api.github.com"],
  auth: {
    token: secrets({
      label: "Personal access token",
      fields: object({ token: string({ minLength: 1 }) }),
    }),
    oauth: oauth2({
      authorizationUrl: "https://github.com/login/oauth/authorize",
      tokenUrl: "https://github.com/login/oauth/access_token",
      tokenEndpointAuthMethod: "client_secret_post",
      scopes: [
        "repo",
        "read:org",
        "workflow",
        "gist",
        "notifications",
        "read:user",
        "user:email",
        "project",
      ],
    }),
  },
  async health({ account, fetch, signal }) {
    const token =
      account.method === "oauth"
        ? account.fields.access_token
        : account.fields.token;
    const response = await fetch("https://api.github.com/user", {
      signal,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
      },
    });
    const me = await decodeJson(response, User);
    return {
      accountInfo: {
        externalId: String(me.id),
        username: me.login,
        avatarUrl: me.avatar_url,
        profileUrl: me.html_url,
      },
    };
  },
});

const requirements = { user: { accounts: { github } } };
type GitHubAccount = AccountOf<typeof github>;
type HookContext = WebhookContext<typeof requirements>;

const empty = object({});
const hookState = object({ hookId: number() });
const apiOrigin = "https://api.github.com";
const repositoryPath = "/repos/qniapp/qni-webgpu";
const targetRepository = "qniapp/qni-webgpu";
const targetRepositoryId = 1128054707;
const maxBodyBytes = 1024 * 1024;
const maxReceipts = 100;
const defaultReceiptLimit = 20;
const maxHookPages = 20;
const apiTimeoutMs = 15_000;
const registrationLeaseMs = 60_000;
const supportedActions = new Set(["opened", "reopened", "synchronize"]);
const deliveryIdPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const signaturePattern = /^sha256=([0-9a-f]{64})$/i;
const headShaPattern = /^[0-9a-f]{40}$/i;

interface ReceiptRow
  extends Record<string, string | number | null | ArrayBuffer> {
  delivery_id: string;
  event: "ping" | "pull_request";
  action: string | null;
  repository: string;
  pull_request_number: number | null;
  head_sha: string | null;
  pull_request_url: string | null;
  received_at: number;
  ping: number;
}

interface ReceiptInput {
  deliveryId: string;
  event: "ping" | "pull_request";
  action: string | null;
  pullRequestNumber: number | null;
  headSha: string | null;
  pullRequestUrl: string | null;
  ping: 0 | 1;
}

interface HookSummary {
  id: number;
  name: string;
  active: boolean;
  events: string[];
  callbackUrl: string;
  contentType: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isTargetRepository(value: unknown): boolean {
  return (
    isRecord(value) &&
    typeof value.id === "number" &&
    Number.isSafeInteger(value.id) &&
    value.id === targetRepositoryId &&
    value.full_name === targetRepository
  );
}

function isDependabotBot(value: unknown): boolean {
  return (
    isRecord(value) && value.login === "dependabot[bot]" && value.type === "Bot"
  );
}

function isPingPayload(payload: Record<string, unknown>): boolean {
  return (
    typeof payload.hook_id === "number" &&
    Number.isSafeInteger(payload.hook_id) &&
    payload.hook_id > 0 &&
    isRecord(payload.hook) &&
    typeof payload.hook.id === "number" &&
    Number.isSafeInteger(payload.hook.id) &&
    payload.hook.id === payload.hook_id
  );
}

function response(status: number): Response {
  return new Response(null, { status });
}

async function readBoundedBody(
  request: Request,
): Promise<
  | { kind: "ok"; bytes: Uint8Array }
  | { kind: "too-large" }
  | { kind: "invalid" }
> {
  if (request.body === null) return { kind: "ok", bytes: new Uint8Array() };
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) break;
      total += chunk.value.byteLength;
      if (total > maxBodyBytes) {
        try {
          await reader.cancel();
        } catch {
          // The body is already being rejected.
        }
        return { kind: "too-large" };
      }
      chunks.push(chunk.value);
    }
  } catch {
    return { kind: "invalid" };
  } finally {
    reader.releaseLock();
  }

  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return { kind: "ok", bytes };
}

function signatureBytes(signature: string): Uint8Array | null {
  const match = signaturePattern.exec(signature);
  if (match === null) return null;
  const hex = match[1];
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function arrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

async function hasValidSignature(
  secret: string,
  signature: string,
  body: Uint8Array,
): Promise<boolean> {
  const bytes = signatureBytes(signature);
  if (bytes === null || secret.length === 0) return false;
  try {
    const key = await crypto.subtle.importKey(
      "raw",
      new TextEncoder().encode(secret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"],
    );
    return await crypto.subtle.verify(
      "HMAC",
      key,
      arrayBuffer(bytes),
      arrayBuffer(body),
    );
  } catch {
    return false;
  }
}

function insertReceipt(sql: Sql, receipt: ReceiptInput): boolean {
  return sql.transaction(
    (transaction) =>
      transaction.exec(
        `INSERT OR IGNORE INTO webhook_receipts
        (delivery_id, event, action, repository, pull_request_number, head_sha, pull_request_url, received_at, ping)
       VALUES (?, ?, ?, ?, CAST(? AS INTEGER), ?, ?, CAST(? AS INTEGER), ?)`,
        receipt.deliveryId,
        receipt.event,
        receipt.action,
        targetRepository,
        receipt.pullRequestNumber,
        receipt.headSha,
        receipt.pullRequestUrl,
        Date.now(),
        receipt.ping,
      ).rowsWritten > 0,
  );
}

function malformed(): Response {
  return response(400);
}

function ignored(): Response {
  return response(202);
}

function storePing(sql: Sql, deliveryId: string): Response {
  insertReceipt(sql, {
    deliveryId,
    event: "ping",
    action: null,
    pullRequestNumber: null,
    headSha: null,
    pullRequestUrl: null,
    ping: 1,
  });
  return ignored();
}

function storePullRequest(
  sql: Sql,
  deliveryId: string,
  action: string,
  pullRequest: Record<string, unknown>,
): Response {
  const number = pullRequest.number;
  const user = pullRequest.user;
  const head = pullRequest.head;
  const base = pullRequest.base;
  if (
    typeof number !== "number" ||
    !Number.isSafeInteger(number) ||
    number < 1 ||
    !isRecord(user) ||
    typeof user.login !== "string" ||
    typeof user.type !== "string" ||
    !isRecord(head) ||
    !isRecord(base) ||
    typeof head.sha !== "string" ||
    !headShaPattern.test(head.sha) ||
    !isRecord(head.repo) ||
    !isRecord(base.repo)
  ) {
    return malformed();
  }
  if (
    !isDependabotBot(user) ||
    !isTargetRepository(head.repo) ||
    !isTargetRepository(base.repo)
  )
    return ignored();

  insertReceipt(sql, {
    deliveryId,
    event: "pull_request",
    action,
    pullRequestNumber: number,
    headSha: head.sha.toLowerCase(),
    pullRequestUrl: `https://github.com/${targetRepository}/pull/${number}`,
    ping: 0,
  });
  return ignored();
}

async function handleIncoming(
  sql: Sql,
  request: Request,
  secret: string,
): Promise<Response> {
  if (request.method !== "POST") return response(405);
  const signature = request.headers.get("x-hub-signature-256");
  if (signature === null || signatureBytes(signature) === null)
    return response(401);

  const contentLength = request.headers.get("content-length");
  if (contentLength !== null) {
    if (!/^\d+$/.test(contentLength)) return malformed();
    const declaredLength = Number(contentLength);
    if (!Number.isSafeInteger(declaredLength)) return malformed();
    if (declaredLength > maxBodyBytes) return response(413);
  }

  const body = await readBoundedBody(request);
  if (body.kind === "too-large") return response(413);
  if (body.kind === "invalid") return malformed();
  if (!(await hasValidSignature(secret, signature, body.bytes)))
    return response(401);

  let payload: unknown;
  try {
    payload = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(body.bytes),
    );
  } catch {
    return malformed();
  }
  if (!isRecord(payload)) return malformed();

  const event = request.headers.get("x-github-event");
  if (event === null) return malformed();
  if (event !== "ping" && event !== "pull_request") return ignored();

  const deliveryHeader = request.headers.get("x-github-delivery");
  if (deliveryHeader === null || !deliveryIdPattern.test(deliveryHeader))
    return malformed();
  const deliveryId = deliveryHeader.toLowerCase();

  if (!isTargetRepository(payload.repository)) return ignored();
  if (event === "ping") {
    return isPingPayload(payload) ? storePing(sql, deliveryId) : ignored();
  }

  if (typeof payload.action !== "string") return malformed();
  if (!supportedActions.has(payload.action)) return ignored();
  if (!isRecord(payload.pull_request)) return malformed();
  return storePullRequest(
    sql,
    deliveryId,
    payload.action,
    payload.pull_request,
  );
}

function safeSubscriptionId(subscriptionId: string): boolean {
  if (subscriptionId.length === 0 || subscriptionId.length > 128) return false;
  for (let index = 0; index < subscriptionId.length; index += 1) {
    const code = subscriptionId.charCodeAt(index);
    if (code <= 0x1f || code === 0x7f) return false;
  }
  return true;
}

function secureCallbackUrl(callbackUrl: string): boolean {
  try {
    const url = new URL(callbackUrl);
    return (
      callbackUrl.length <= 2048 &&
      url.protocol === "https:" &&
      url.username === "" &&
      url.password === "" &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
}

function acquireRegistrationLease(
  sql: Sql,
  subscriptionId: string,
  token: string,
): boolean {
  const now = Date.now();
  const leaseUntil = now + registrationLeaseMs;
  return sql.transaction(
    (transaction) =>
      transaction.exec(
        `INSERT INTO github_hook_registrations (subscription_id, hook_id, lease_token, lease_until)
       VALUES (?, NULL, ?, CAST(? AS INTEGER))
       ON CONFLICT(subscription_id) DO UPDATE
         SET lease_token = excluded.lease_token, lease_until = excluded.lease_until
         WHERE github_hook_registrations.lease_token IS NULL
            OR github_hook_registrations.lease_until <= CAST(? AS INTEGER)`,
        subscriptionId,
        token,
        leaseUntil,
        now,
      ).rowsWritten > 0,
  );
}

function readRecordedHookId(
  sql: SqlReader,
  subscriptionId: string,
): number | null {
  const rows = sql
    .exec<{
      hook_id: number | null;
    }>(
      "SELECT hook_id FROM github_hook_registrations WHERE subscription_id = ?",
      subscriptionId,
    )
    .toArray();
  return rows[0]?.hook_id ?? null;
}

function rememberHookId(
  sql: Sql,
  subscriptionId: string,
  token: string,
  hookId: number,
): boolean {
  return (
    sql.exec(
      `UPDATE github_hook_registrations
       SET hook_id = CAST(? AS INTEGER)
       WHERE subscription_id = ? AND lease_token = ?`,
      hookId,
      subscriptionId,
      token,
    ).rowsWritten > 0
  );
}

function releaseRegistrationLease(
  sql: Sql,
  subscriptionId: string,
  token: string,
): void {
  sql.transaction((transaction) => {
    transaction.exec(
      `UPDATE github_hook_registrations
       SET lease_token = NULL, lease_until = NULL
       WHERE subscription_id = ? AND lease_token = ?`,
      subscriptionId,
      token,
    );
    transaction.exec(
      `DELETE FROM github_hook_registrations
       WHERE subscription_id = ? AND hook_id IS NULL AND lease_token IS NULL`,
      subscriptionId,
    );
  });
}

function forgetRegistration(
  sql: Sql,
  subscriptionId: string,
  token: string,
): void {
  sql.exec(
    "DELETE FROM github_hook_registrations WHERE subscription_id = ? AND lease_token = ?",
    subscriptionId,
    token,
  );
}

function apiSignal(signal: AbortSignal): AbortSignal {
  return AbortSignal.any([signal, AbortSignal.timeout(apiTimeoutMs)]);
}

async function githubRequest(
  context: HookContext,
  account: GitHubAccount,
  path: string,
  method: "GET" | "POST" | "PATCH" | "DELETE",
  signal: AbortSignal,
  body?: unknown,
): Promise<Response> {
  const token =
    account.method === "oauth"
      ? account.fields.access_token
      : account.fields.token;
  try {
    return await context.fetch(`${apiOrigin}${path}`, {
      method,
      redirect: "manual",
      cache: "no-store",
      signal,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "qni-executor-pr-inbox",
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
  } catch {
    throw new Error("GitHub API request failed.");
  }
}

function requireStatus(
  response: Response,
  expected: number | readonly number[],
): void {
  if (response.status >= 300 && response.status < 400)
    throw new Error("GitHub API redirected the request.");
  const accepted = typeof expected === "number" ? [expected] : expected;
  if (!accepted.includes(response.status))
    throw new Error(`GitHub API returned HTTP ${response.status}.`);
}

async function responseJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    throw new Error("GitHub API returned invalid JSON.");
  }
}

function parseHook(value: unknown): HookSummary {
  if (
    !isRecord(value) ||
    typeof value.id !== "number" ||
    !Number.isSafeInteger(value.id) ||
    value.id < 1 ||
    typeof value.name !== "string" ||
    typeof value.active !== "boolean" ||
    !Array.isArray(value.events) ||
    !value.events.every((event) => typeof event === "string") ||
    !isRecord(value.config) ||
    typeof value.config.url !== "string" ||
    typeof value.config.content_type !== "string"
  ) {
    throw new Error("GitHub API returned an invalid webhook record.");
  }
  return {
    id: value.id,
    name: value.name,
    active: value.active,
    events: value.events,
    callbackUrl: value.config.url,
    contentType: value.config.content_type,
  };
}

function isConfiguredHook(hook: HookSummary, callbackUrl: string): boolean {
  return (
    hook.name === "web" &&
    hook.active &&
    hook.events.length === 1 &&
    hook.events[0] === "pull_request" &&
    hook.callbackUrl === callbackUrl &&
    hook.contentType === "json"
  );
}

async function listHooksForCallback(
  context: HookContext,
  account: GitHubAccount,
  callbackUrl: string,
  signal: AbortSignal,
): Promise<HookSummary[]> {
  const matches: HookSummary[] = [];
  for (let page = 1; page <= maxHookPages; page += 1) {
    const response = await githubRequest(
      context,
      account,
      `${repositoryPath}/hooks?per_page=100&page=${page}`,
      "GET",
      signal,
    );
    requireStatus(response, 200);
    const value = await responseJson(response);
    if (!Array.isArray(value))
      throw new Error("GitHub API returned an invalid webhook list.");
    const hooks = value.map(parseHook);
    matches.push(...hooks.filter((hook) => hook.callbackUrl === callbackUrl));
    if (hooks.length < 100) return matches;
  }
  throw new Error("GitHub returned too many webhook pages to inspect safely.");
}

async function verifySavedHook(
  response: Response,
  expectedId: number,
  callbackUrl: string,
): Promise<number> {
  const hook = parseHook(await responseJson(response));
  if (hook.id !== expectedId || !isConfiguredHook(hook, callbackUrl)) {
    throw new Error("GitHub did not save the expected webhook configuration.");
  }
  return hook.id;
}

async function ensureHook(
  context: HookContext,
  account: GitHubAccount,
  callbackUrl: string,
  secret: string,
  signal: AbortSignal,
  onCreateAttempt: () => void,
  onCreateConfirmed: () => void,
): Promise<number> {
  const matches = await listHooksForCallback(
    context,
    account,
    callbackUrl,
    signal,
  );
  if (matches.length > 1)
    throw new Error(
      "More than one GitHub webhook uses this Executor callback URL.",
    );

  const body = {
    name: "web",
    active: true,
    events: ["pull_request"],
    config: {
      url: callbackUrl,
      content_type: "json",
      secret,
      insecure_ssl: "0",
    },
  };
  if (matches.length === 1) {
    const existing = matches[0];
    const update = await githubRequest(
      context,
      account,
      `${repositoryPath}/hooks/${existing.id}`,
      "PATCH",
      signal,
      body,
    );
    requireStatus(update, 200);
    return verifySavedHook(update, existing.id, callbackUrl);
  }

  onCreateAttempt();
  const created = await githubRequest(
    context,
    account,
    `${repositoryPath}/hooks`,
    "POST",
    signal,
    body,
  );
  requireStatus(created, 201);
  const value = parseHook(await responseJson(created));
  if (!isConfiguredHook(value, callbackUrl))
    throw new Error("GitHub did not create the expected webhook.");
  onCreateConfirmed();
  return value.id;
}

function validHookState(state: unknown): number | null {
  if (
    !isRecord(state) ||
    typeof state.hookId !== "number" ||
    !Number.isSafeInteger(state.hookId) ||
    state.hookId < 1
  ) {
    return null;
  }
  return state.hookId;
}

const githubWebhook = {
  account: { user: "github" },
  config: empty,
  state: hookState,
  async register(context, { account, callbackUrl, secret, subscriptionId }) {
    if (
      !safeSubscriptionId(subscriptionId) ||
      !secureCallbackUrl(callbackUrl) ||
      secret.length === 0
    ) {
      throw new Error(
        "Executor supplied invalid webhook registration details.",
      );
    }
    const leaseToken = crypto.randomUUID();
    if (!acquireRegistrationLease(context.sql, subscriptionId, leaseToken)) {
      throw new Error(
        "GitHub webhook registration is already in progress; reconcile and retry.",
      );
    }

    let createAttempted = false;
    let createConfirmed = false;
    try {
      const hookId = await ensureHook(
        context,
        account,
        callbackUrl,
        secret,
        apiSignal(context.signal),
        () => {
          createAttempted = true;
        },
        () => {
          createConfirmed = true;
        },
      );
      if (!rememberHookId(context.sql, subscriptionId, leaseToken, hookId)) {
        throw new Error("GitHub webhook registration lease was lost.");
      }
      return { hookId };
    } catch {
      throw new Error(
        "GitHub webhook registration failed; check repository permissions or callback URL ambiguity, then reconcile.",
      );
    } finally {
      if (!createAttempted || createConfirmed) {
        releaseRegistrationLease(context.sql, subscriptionId, leaseToken);
      }
    }
  },
  async handle(context, { request, secret }) {
    return handleIncoming(context.sql, request, secret);
  },
  async unregister(context, { account, state, subscriptionId }) {
    if (!safeSubscriptionId(subscriptionId))
      throw new Error("Executor supplied an invalid webhook subscription ID.");
    const leaseToken = crypto.randomUUID();
    if (!acquireRegistrationLease(context.sql, subscriptionId, leaseToken)) {
      throw new Error(
        "GitHub webhook registration is already in progress; reconcile and retry.",
      );
    }
    try {
      const stateHookId = validHookState(state);
      const recordedHookId = readRecordedHookId(context.sql, subscriptionId);
      if (
        stateHookId !== null &&
        recordedHookId !== null &&
        stateHookId !== recordedHookId
      ) {
        throw new Error(
          "GitHub webhook state does not match its recorded hook.",
        );
      }
      const hookId = stateHookId ?? recordedHookId;
      if (hookId === null) {
        forgetRegistration(context.sql, subscriptionId, leaseToken);
        return;
      }
      if (
        recordedHookId === null &&
        !rememberHookId(context.sql, subscriptionId, leaseToken, hookId)
      ) {
        throw new Error("GitHub webhook registration lease was lost.");
      }

      const remove = await githubRequest(
        context,
        account,
        `${repositoryPath}/hooks/${hookId}`,
        "DELETE",
        apiSignal(context.signal),
      );
      requireStatus(remove, [204, 404]);
      forgetRegistration(context.sql, subscriptionId, leaseToken);
    } catch {
      throw new Error("GitHub webhook cleanup failed; reconcile and retry.");
    } finally {
      releaseRegistrationLease(context.sql, subscriptionId, leaseToken);
    }
  },
} satisfies Webhook<HookContext, typeof empty, typeof hookState>;

export { githubWebhook };

export function readRecentReceipts(
  sql: SqlReader,
  requestedLimit?: number,
): ReceiptRow[] {
  const limit =
    typeof requestedLimit === "number" && Number.isSafeInteger(requestedLimit)
      ? Math.min(maxReceipts, Math.max(1, requestedLimit))
      : defaultReceiptLimit;
  return sql
    .exec<ReceiptRow>(
      `SELECT delivery_id, event, action, repository, pull_request_number, head_sha, pull_request_url, received_at, ping
       FROM webhook_receipts
       ORDER BY received_at DESC, delivery_id ASC
       LIMIT CAST(? AS INTEGER)`,
      limit,
    )
    .toArray();
}

const recentReceipts = query(
  {
    title: "GitHub webhook の受信記録",
    description:
      "署名済み ping と Dependabot のプルリクエスト通知を最近の順に読みます。",
    input: object({ limit: number().optional() }),
  },
  async (context: QueryContext<typeof requirements>, { limit }) =>
    readRecentReceipts(context.sql, limit),
);

export default defineApp(requirements, {
  tools: router({ recentReceipts }),
  webhooks: { github: githubWebhook },
});
