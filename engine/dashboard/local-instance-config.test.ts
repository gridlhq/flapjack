import { describe, expect, it } from 'vitest';

import { parseLocalConfigFile } from './local-instance-config';

describe('parseLocalConfigFile', () => {
  it('parses plain KEY=value assignments', () => {
    const parsed = parseLocalConfigFile([
      'FJ_HOST=127.0.0.1',
      'FJ_BACKEND_PORT=18893',
      'FJ_DASHBOARD_PORT=15183',
      '',
    ].join('\n'));

    expect(parsed).toEqual({
      FJ_HOST: '127.0.0.1',
      FJ_BACKEND_PORT: '18893',
      FJ_DASHBOARD_PORT: '15183',
    });
  });

  it('parses export syntax and strips inline comments from unquoted values', () => {
    const parsed = parseLocalConfigFile([
      'export FJ_HOST=127.0.0.1',
      'export FJ_BACKEND_PORT=18893 # backend',
      'export FJ_DASHBOARD_PORT=15183    # dashboard',
      'export FJ_TEST_ADMIN_KEY=fj_dev_key # test key',
    ].join('\n'));

    expect(parsed).toEqual({
      FJ_HOST: '127.0.0.1',
      FJ_BACKEND_PORT: '18893',
      FJ_DASHBOARD_PORT: '15183',
      FJ_TEST_ADMIN_KEY: 'fj_dev_key',
    });
  });

  it('keeps # characters inside quoted values', () => {
    const parsed = parseLocalConfigFile([
      'FJ_TEST_ADMIN_KEY="key-with-#-inside"',
      "FJ_HOST='127.0.0.1#suffix'",
    ].join('\n'));

    expect(parsed).toEqual({
      FJ_TEST_ADMIN_KEY: 'key-with-#-inside',
      FJ_HOST: '127.0.0.1#suffix',
    });
  });

  it('ignores comments, blank lines, and malformed lines', () => {
    const parsed = parseLocalConfigFile([
      '# comment',
      '',
      'NOT_AN_ASSIGNMENT',
      'export',
      '=missing_key',
      'FJ_HOST=0.0.0.0',
    ].join('\n'));

    expect(parsed).toEqual({
      FJ_HOST: '0.0.0.0',
    });
  });
});
