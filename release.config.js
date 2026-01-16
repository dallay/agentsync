module.exports = {
  branches: [
    'main',
    { name: 'beta', prerelease: true },
    { name: 'alpha', prerelease: true }
  ],
  plugins: [
    [
      '@semantic-release/exec',
      {
        prepareCmd: 'node ./scripts/update-optional-deps.js ${nextRelease.version}'
      }
    ],
    [
      '@semantic-release/git',
      {
        assets: ['npm/agentsync/package.json'],
        message: 'chore(release): bump package for ${nextRelease.version} [skip ci]'
      }
    ],
    '@semantic-release/npm',
    '@semantic-release/github'
  ]
};
