import assert from "node:assert/strict";
import { createHmac, randomBytes } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { DatabaseSync } from "node:sqlite";
import { githubWebhook, readRecentReceipts } from "../index.ts";

const fixtureSecret = randomBytes(32).toString("base64url");
const fixtureToken = randomBytes(32).toString("hex");
const targetRepository = { id: 1128054707, full_name: "qniapp/qni-webgpu" };
const targetUrl = "https://github.com/qniapp/qni-webgpu/pull/42";

async function createContext() {
  const database = new DatabaseSync(":memory:");
  const migration = await readFile(
    new URL("../migrations/0001_webhook_receipts.sql", import.meta.url),
    "utf8",
  );
  database.exec(migration);

  const sql = {
    exec(statement, ...bindings) {
      const prepared = database.prepare(statement);
      if (prepared.columns().length > 0) {
        const rows = prepared.all(...bindings);
        return {
          rowsRead: rows.length,
          rowsWritten: 0,
          toArray: () => rows,
          one: () => {
            if (rows.length !== 1)
              throw new Error(`expected one row, found ${rows.length}`);
            return rows[0];
          },
          [Symbol.iterator]: () => rows[Symbol.iterator](),
        };
      }
      const result = prepared.run(...bindings);
      return {
        rowsRead: 0,
        rowsWritten: Number(result.changes),
        toArray: () => [],
        one: () => {
          throw new Error("statement returned no rows");
        },
        [Symbol.iterator]: () => [][Symbol.iterator](),
      };
    },
    transaction(work) {
      database.exec("BEGIN IMMEDIATE");
      try {
        const result = work(sql);
        database.exec("COMMIT");
        return result;
      } catch (error) {
        database.exec("ROLLBACK");
        throw error;
      }
    },
  };

  return {
    database,
    sql,
    context: { sql, signal: new AbortController().signal },
  };
}

function makePullRequest(overrides = {}) {
  return {
    action: "opened",
    repository: { ...targetRepository },
    pull_request: {
      number: 42,
      user: { login: "dependabot[bot]", type: "Bot" },
      head: {
        sha: "0123456789abcdef0123456789abcdef01234567",
        repo: { ...targetRepository },
      },
      base: { repo: { ...targetRepository } },
    },
    ...overrides,
  };
}

function signedRequest(
  payload,
  {
    event = "pull_request",
    delivery = "11111111-2222-4333-8444-555555555555",
    rawBody,
  } = {},
) {
  const body = rawBody ?? JSON.stringify(payload);
  const signature = createHmac("sha256", fixtureSecret)
    .update(Buffer.from(body))
    .digest("hex");
  return new Request("https://executor.invalid/webhook", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-github-delivery": delivery,
      "x-github-event": event,
      "x-hub-signature-256": `sha256=${signature}`,
    },
    body,
  });
}

async function deliver(context, request) {
  return githubWebhook.handle(context, {
    request,
    config: {},
    subscriptionId: "subscription-fixture",
    account: { method: "token", fields: { token: fixtureToken } },
    callbackUrl: "https://executor.invalid/webhook",
    state: null,
    secret: fixtureSecret,
  });
}

function receiptRows(sql) {
  return sql
    .exec(
      "SELECT delivery_id, event, action, repository, pull_request_number, head_sha, pull_request_url, received_at, ping FROM webhook_receipts ORDER BY received_at DESC, delivery_id ASC",
    )
    .toArray();
}

test("stores only metadata from a signed Dependabot pull request", async () => {
  const { context, sql } = await createContext();
  const response = await deliver(context, signedRequest(makePullRequest()));
  const rows = receiptRows(sql);

  assert.deepEqual(
    {
      status: response.status,
      rows: rows.map(({ received_at, ...row }) => row),
    },
    {
      status: 202,
      rows: [
        {
          delivery_id: "11111111-2222-4333-8444-555555555555",
          event: "pull_request",
          action: "opened",
          repository: "qniapp/qni-webgpu",
          pull_request_number: 42,
          head_sha: "0123456789abcdef0123456789abcdef01234567",
          pull_request_url: targetUrl,
          ping: 0,
        },
      ],
    },
  );
});

test("deduplicates concurrent redelivery of the same signed event", async () => {
  const { context, sql } = await createContext();
  const responses = await Promise.all([
    deliver(context, signedRequest(makePullRequest())),
    deliver(context, signedRequest(makePullRequest())),
  ]);

  assert.deepEqual(
    {
      statuses: responses.map(({ status }) => status),
      count: receiptRows(sql).length,
    },
    { statuses: [202, 202], count: 1 },
  );
});

test("checks the HMAC against the original request bytes", async () => {
  const { context, sql } = await createContext();
  const rawBody = ` ${JSON.stringify(makePullRequest())}\n`;
  const response = await deliver(context, signedRequest(null, { rawBody }));

  assert.deepEqual(
    { status: response.status, count: receiptRows(sql).length },
    { status: 202, count: 1 },
  );
});

test("stores a minimal signed ping receipt", async () => {
  const { context, sql } = await createContext();
  const response = await deliver(
    context,
    signedRequest(
      {
        repository: { ...targetRepository },
        hook_id: 987,
        hook: { id: 987, type: "Repository" },
        zen: "not retained",
      },
      { event: "ping" },
    ),
  );
  const rows = receiptRows(sql);

  assert.deepEqual(
    {
      status: response.status,
      row: rows.map(({ received_at, ...row }) => row)[0],
      count: rows.length,
    },
    {
      status: 202,
      row: {
        delivery_id: "11111111-2222-4333-8444-555555555555",
        event: "ping",
        action: null,
        repository: "qniapp/qni-webgpu",
        pull_request_number: null,
        head_sha: null,
        pull_request_url: null,
        ping: 1,
      },
      count: 1,
    },
  );
});

test("rejects missing and incorrect signatures without writes", async () => {
  const { context, sql } = await createContext();
  const valid = signedRequest(makePullRequest());
  const missing = new Request(valid.url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-github-delivery": "11111111-2222-4333-8444-555555555555",
      "x-github-event": "pull_request",
    },
    body: JSON.stringify(makePullRequest()),
  });
  const incorrect = new Request(valid.url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-github-delivery": "11111111-2222-4333-8444-555555555555",
      "x-github-event": "pull_request",
      "x-hub-signature-256": `sha256=${"0".repeat(64)}`,
    },
    body: JSON.stringify(makePullRequest()),
  });
  const responses = await Promise.all([
    deliver(context, missing),
    deliver(context, incorrect),
  ]);

  assert.deepEqual(
    {
      statuses: responses.map(({ status }) => status),
      count: receiptRows(sql).length,
    },
    { statuses: [401, 401], count: 0 },
  );
});

test("ignores unsupported events, actions, authors, and repository identities", async () => {
  const { context, sql } = await createContext();
  const payloads = [
    { event: "issues", payload: { repository: { ...targetRepository } } },
    { event: "ping", payload: makePullRequest() },
    { event: "pull_request", payload: makePullRequest({ action: "closed" }) },
    {
      event: "pull_request",
      payload: makePullRequest({
        pull_request: {
          ...makePullRequest().pull_request,
          user: { login: "octocat", type: "User" },
        },
      }),
    },
    {
      event: "pull_request",
      payload: makePullRequest({
        repository: { id: 1, full_name: "someone/else" },
      }),
    },
    {
      event: "pull_request",
      payload: makePullRequest({
        pull_request: {
          ...makePullRequest().pull_request,
          head: {
            ...makePullRequest().pull_request.head,
            repo: { id: 1, full_name: "someone/else" },
          },
        },
      }),
    },
    {
      event: "pull_request",
      payload: makePullRequest({
        pull_request: {
          ...makePullRequest().pull_request,
          base: {
            repo: { id: 1, full_name: "someone/else" },
          },
        },
      }),
    },
  ];
  const responses = await Promise.all(
    payloads.map(({ event, payload }, index) =>
      deliver(
        context,
        signedRequest(payload, {
          event,
          delivery: `00000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
        }),
      ),
    ),
  );

  assert.deepEqual(
    {
      statuses: responses.map(({ status }) => status),
      count: receiptRows(sql).length,
    },
    { statuses: [202, 202, 202, 202, 202, 202, 202], count: 0 },
  );
});

test("refuses malformed signed JSON without writes", async () => {
  const { context, sql } = await createContext();
  const response = await deliver(
    context,
    signedRequest(null, { rawBody: "{not-json" }),
  );

  assert.deepEqual(
    { status: response.status, count: receiptRows(sql).length },
    { status: 400, count: 0 },
  );
});

test("rejects an oversized signed body without writes", async () => {
  const { context, sql } = await createContext();
  const oversized = "x".repeat(1024 * 1024 + 1);
  const response = await deliver(
    context,
    signedRequest(null, { rawBody: oversized }),
  );

  assert.deepEqual(
    { status: response.status, count: receiptRows(sql).length },
    { status: 413, count: 0 },
  );
});

test("bounds recent receipt queries to the newest 100 rows", async () => {
  const { sql } = await createContext();
  for (let index = 0; index < 120; index += 1) {
    sql.exec(
      "INSERT INTO webhook_receipts (delivery_id, event, action, repository, pull_request_number, head_sha, pull_request_url, received_at, ping) VALUES (?, 'ping', NULL, 'qniapp/qni-webgpu', NULL, NULL, NULL, ?, 1)",
      `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
      index,
    );
  }
  const rows = readRecentReceipts(sql, 1000);

  assert.deepEqual(
    {
      count: rows.length,
      newest: rows[0].received_at,
      oldest: rows.at(-1).received_at,
    },
    { count: 100, newest: 119, oldest: 20 },
  );
});

test("register retries reuse the exact hook and unregister deletes only its recorded ID", async () => {
  const hooks = [];
  const calls = [];
  const context = {
    sql: (await createContext()).sql,
    signal: new AbortController().signal,
    async fetch(input, init) {
      const url = new URL(input);
      const method = init.method ?? "GET";
      calls.push(`${method} ${url.pathname}`);
      if (url.pathname.endsWith("/hooks") && method === "GET")
        return Response.json(hooks);
      if (url.pathname.endsWith("/hooks") && method === "POST") {
        const body = JSON.parse(init.body);
        hooks.push({
          id: 77,
          name: "web",
          active: true,
          events: body.events,
          config: {
            url: body.config.url,
            content_type: body.config.content_type,
          },
        });
        throw new Error("simulated interrupted response");
      }
      if (url.pathname.endsWith("/hooks/77") && method === "PATCH") {
        const body = JSON.parse(init.body);
        hooks[0].events = body.events;
        hooks[0].config.url = body.config.url;
        hooks[0].config.content_type = body.config.content_type;
        return Response.json(hooks[0]);
      }
      if (url.pathname.endsWith("/hooks/77") && method === "DELETE") {
        hooks.splice(0, hooks.length);
        return new Response(null, { status: 204 });
      }
      throw new Error(
        `unexpected fake GitHub request: ${method} ${url.pathname}`,
      );
    },
  };
  const input = {
    config: {},
    subscriptionId: "subscription-fixture",
    account: { method: "token", fields: { token: fixtureToken } },
    callbackUrl: "https://executor.invalid/webhook",
    secret: fixtureSecret,
  };
  let firstRegistrationFailed = false;
  try {
    await githubWebhook.register(context, input);
  } catch {
    firstRegistrationFailed = true;
  }
  let prematureRetryRejected = false;
  try {
    await githubWebhook.register(context, input);
  } catch {
    prematureRetryRejected = true;
  }
  context.sql.exec(
    "UPDATE github_hook_registrations SET lease_until = 0 WHERE subscription_id = ?",
    input.subscriptionId,
  );
  const retry = await githubWebhook.register(context, input);
  let mismatchedCleanupFailed = false;
  try {
    await githubWebhook.unregister(context, {
      ...input,
      state: { hookId: 78 },
    });
  } catch {
    mismatchedCleanupFailed = true;
  }
  const deletesBeforeSafeCleanup = calls.filter((call) =>
    call.startsWith("DELETE "),
  );
  await githubWebhook.unregister(context, { ...input, state: null });

  assert.deepEqual(
    {
      firstRegistrationFailed,
      prematureRetryRejected,
      retry,
      mismatchedCleanupFailed,
      deletesBeforeSafeCleanup,
      hookCount: hooks.length,
      postCount: calls.filter((call) => call.startsWith("POST ")).length,
      patchCount: calls.filter((call) => call.startsWith("PATCH ")).length,
      deletes: calls.filter((call) => call.startsWith("DELETE ")),
    },
    {
      firstRegistrationFailed: true,
      prematureRetryRejected: true,
      retry: { hookId: 77 },
      mismatchedCleanupFailed: true,
      deletesBeforeSafeCleanup: [],
      hookCount: 0,
      postCount: 1,
      patchCount: 1,
      deletes: ["DELETE /repos/qniapp/qni-webgpu/hooks/77"],
    },
  );
});
