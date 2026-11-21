import { expect, test } from "bun:test";
import { signOut } from "../src/logout";

test("only confirmed revocation leaves the session", async () => {
  let departures = 0;
  const leave = () => { departures++; };
  for (const response of [new Response(null, { status: 503 }), Response.json({state: "pending"})]) {
    await expect(signOut(leave, async () => response)).rejects.toThrow();
  }
  expect(departures).toBe(0);
  await signOut(leave, async () => Response.json({state: "revoked"}));
  expect(departures).toBe(1);
});
