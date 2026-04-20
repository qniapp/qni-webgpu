module.exports = {
  paths: ['features/**/*.feature.md'],
  require: ['features/support/**/*.cjs', 'features/step_definitions/**/*.cjs'],
  publishQuiet: true,
  failFast: true,
}
