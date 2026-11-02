import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { builtinSchema, fromBytes } from '../src/schema.ts';
import { encodeRequest, decodeRequest, encodeResponse, decodeResponse } from '../src/codec.ts';

const schemas = [['built-in', builtinSchema()]] as const;
const cases = [...schemas] as Array<readonly [string, ReturnType<typeof builtinSchema>]>;
if (process.env.BP_KICAD_DESCRIPTOR) cases.push(['runtime descriptor', fromBytes(readFileSync(process.env.BP_KICAD_DESCRIPTOR))]);
for (const [name, schema] of cases) {
  test(`${name}: API request and typed response preserve the envelope`, () => {
    const type = 'kiapi.common.commands.GetVersion';
    const request = decodeRequest(schema, encodeRequest(schema, type, {}, 'synthetic-token', 'fixture'));
    assert.equal(request.type, type);
    assert.equal(request.token, 'synthetic-token');
    const answer = decodeResponse(schema, encodeResponse(schema, {
      type: 'kiapi.common.commands.GetVersionResponse',
      message: { version: { major: 10, minor: 99, patch: 0, full_version: 'synthetic' } },
      token: 'synthetic-token',
    }));
    assert.equal(answer.ok, true);
    if (answer.ok) assert.equal((answer.message as { version: { major: number } }).version.major, 10);
    assert.equal(answer.token, 'synthetic-token');
  });
  test(`${name}: API failures retain their status and diagnostic`, () => {
    const answer = decodeResponse(schema, encodeResponse(schema, { status: 3, error: 'synthetic refusal', token: 'synthetic-token' }));
    assert.equal(answer.ok, false);
    if (!answer.ok) {
      assert.equal(answer.status, 'AS_BAD_REQUEST');
      assert.equal(answer.error, 'synthetic refusal');
    }
  });
}
