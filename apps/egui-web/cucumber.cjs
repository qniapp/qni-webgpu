const defaultProfile = {
  paths: ['features/**/*.feature.md'],
  requireModule: ['ts-node/register'],
  require: [
    'features/support/bootstrap.ts',
    'features/step_definitions/**/*.cjs',
    'features/step_definitions/**/*.ts',
  ],
  publishQuiet: true,
  failFast: true,
  parallel: process.env.CI ? 2 : undefined,
}

module.exports = {
  default: defaultProfile,
}
