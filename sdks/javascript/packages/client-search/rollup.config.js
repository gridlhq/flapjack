export default [
  {
    input: 'dist/builds/browser.min.js',
    external: ['dom'],
    cache: false,
    output: {
      esModule: false,
      file: 'dist/builds/browser.umd.js',
      name: '@flapjack-search/client-search',
      format: 'umd',
      sourcemap: false,
      globals: {
        ['searchClient']: 'searchClient',
      },
    },
  },
]