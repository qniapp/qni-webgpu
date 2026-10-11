import { expect, test, type BrowserContext } from '@playwright/test'
import { readStateVector, waitForStartupReady } from './support/web-spec-helpers'

test.use({ viewport: { width: 1280, height: 800 } })

// 同じ列で同じ角度の P ゲートは 1 つの多重制御位相 (CPHASE / CCPHASE) になる
// (issue #57、qni チュートリアル `cphase.html`)。各ケースは、同じ演算を
// コントロールゲートで書いた回路と同じ状態ベクトルになることを確かめる。
const PLUS3 = '["H","H","H"]'
const circuit = (...cols: string[]): string => `{"cols":[${[PLUS3, ...cols].join(',')}]}`

// hash だけが違う URL への goto はアプリを読み込み直さないので、回路ごとに
// 新しいページで開く。起動完了後の状態ベクトルは確定しているので一度だけ読む。
// expect.poll で包むと、並列実行で 1 回の起動が既定の 5 秒を超えたときに落ちる。
const stateVectorOf = async (context: BrowserContext, json: string): Promise<number[]> => {
  const page = await context.newPage()
  try {
    await page.goto(`/#${encodeURIComponent(json)}`)
    await waitForStartupReady(page, { waitForStateVector: true })
    // P(π) の虚部は f32 の sin(π) で -0 に丸まることがあるので 0 にそろえる。
    return (await readStateVector(page) as number[]).map((value) => Math.round(value * 1e4) / 1e4 || 0)
  } finally {
    await page.close()
  }
}

const cases: { name: string; phases: string; controlled: string[] }[] = [
  {
    name: 'two same-angle P gates act as the tutorial CPHASE',
    phases: '["P(π_4)",1,"P(π_4)"]',
    controlled: ['["•",1,"P(π_4)"]'],
  },
  {
    name: 'three same-angle P gates act as CCPHASE',
    phases: '["P(π_4)","P(π_4)","P(π_4)"]',
    controlled: ['["•","•","P(π_4)"]'],
  },
  {
    name: 'a control joins the CPHASE condition',
    phases: '["P(π_4)","•","P(π_4)"]',
    controlled: ['["•","•","P(π_4)"]'],
  },
  {
    name: 'an anti-control joins the CPHASE condition',
    phases: '["P(π_4)","◦","P(π_4)"]',
    controlled: ['["•","◦","P(π_4)"]'],
  },
  {
    name: 'P gates of different angles form separate groups',
    phases: '["P(π_4)","P(π_2)","P(π_4)"]',
    controlled: ['["•",1,"P(π_4)"]', '[1,"P(π_2)"]'],
  },
  {
    name: 'two P(π) gates act as CZ',
    phases: '["P(π)","P(π)"]',
    controlled: ['["•","•"]'],
  },
]

for (const { name, phases, controlled } of cases) {
  test(name, async ({ context }) => {
    const expected = await stateVectorOf(context, circuit(...controlled))

    expect(await stateVectorOf(context, circuit(phases))).toEqual(expected)
  })
}

test('CPHASE changes only the |101⟩ and |111⟩ amplitudes', async ({ context }) => {
  // |+++⟩ に CPHASE(π/4) をかけると、q0 と q2 がともに 1 の振幅だけ e^{iπ/4} 倍になる。
  const s = Math.round(1e4 / Math.sqrt(8)) / 1e4
  const r = 0.25
  const expected = [s, 0, s, 0, s, 0, s, 0, s, 0, r, r, s, 0, r, r]

  expect(await stateVectorOf(context, circuit('["P(π_4)",1,"P(π_4)"]'))).toEqual(expected)
})
