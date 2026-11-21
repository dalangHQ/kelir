// see: cans/08-lifecycle.md#Detached-lifecycle

import type { Session } from "./session.lifecycle.flow.js";
import { EConfig } from "./errors.sdk.flow.js";

// TODO (L3) [lc_15_detached_session_reconnect_relay]: connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.; slop: attach skips lease check, binds dead relay.sock
export async function sessionAttach(sessionId: string): Promise<Session> {
  void sessionId;
  throw new EConfig("TODO: Session.attach");
}
