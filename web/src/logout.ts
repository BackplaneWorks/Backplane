export async function signOut(leaveSession: () => void, request = fetch): Promise<void> {
  const response = await request("/auth/web/logout", {
    method: "POST",
    credentials: "same-origin",
    redirect: "error",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
  if (response.status !== 200 || (await response.json()).state !== "revoked") {
    throw new Error("Logout was not confirmed");
  }
  leaveSession();
}
