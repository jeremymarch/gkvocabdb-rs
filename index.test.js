// eslint-disable-next-line no-unused-vars
/* global require expect test describe */
// import * as blah from "scripts/index.js";
const mod = require('./static/index.js');

function add (a, b) {
  return a + b;
}

describe('Test Testing', () => {
  test('1 + 2 === 3', () => {
    expect(add(1, 2)).toBe(3);
  });
  test('2 - 1 === 1', () => {
    expect(mod.subtract(2, 1)).toBe(1);
  });
});
