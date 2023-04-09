module.exports = {
  env: {
    browser: true
  },
  plugins: ['html'],
  extends: 'standard',
  rules: {
    semi: [2, 'always']
  },
  ignorePatterns: ['static/hoplitekb_wasm_rs.js'],
  overrides: [
  ],
  parserOptions: {
    ecmaVersion: 'latest'
  }
};
