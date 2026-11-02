export type TunnelState = "off" | "starting" | "ready" | "missing" | "failed";

export type MobileStatus = {
  running: boolean;
  tunnel: {
    state: TunnelState;
    publicUrl: string | null;
    link: string | null;
    qrSvg: string | null;
    error: string | null;
  };
  lastClientMs: number | null;
};

export const MOBILE_STATUS_EVENT = "terax:mobile-status";

export const MOBILE_OFF: MobileStatus = {
  running: false,
  tunnel: {
    state: "off",
    publicUrl: null,
    link: null,
    qrSvg: null,
    error: null,
  },
  lastClientMs: null,
};

const CLIENT_ACTIVE_MS = 15_000;

export function phoneConnected(status: MobileStatus, now: number): boolean {
  return (
    status.running &&
    status.lastClientMs !== null &&
    now - status.lastClientMs < CLIENT_ACTIVE_MS
  );
}
