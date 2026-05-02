type CucumberProfile = {
  paths: string[]
  requireModule: string[]
  require: string[]
  publishQuiet: boolean
  failFast: boolean
  parallel?: number
}

const defaultProfile: CucumberProfile = {
  paths: ['features/**/*.feature.md'],
  requireModule: ['ts-node/register'],
  require: [
    'features/support/bootstrap.ts',
    'features/step_definitions/**/*.ts',
  ],
  publishQuiet: true,
  failFast: true,
  parallel: process.env.CI ? 2 : undefined,
}

module.exports = defaultProfile
