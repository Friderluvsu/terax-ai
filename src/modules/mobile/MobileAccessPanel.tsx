import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { type MobileStatus, phoneConnected } from "./lib/status";

const INSTALL_COMMAND = "brew install cloudflared";

type Props = {
  status: MobileStatus;
  onStatus: (status: MobileStatus) => void;
};

function useCopy(): [string | null, (key: string, text: string) => void] {
  const [copied, setCopied] = useState<string | null>(null);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(null), 1500);
    return () => clearTimeout(timer);
  }, [copied]);
  return [
    copied,
    (key, text) => {
      void navigator.clipboard.writeText(text).then(() => setCopied(key));
    },
  ];
}

export default function MobileAccessPanel({ status, onStatus }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [copied, copy] = useCopy();

  useEffect(() => {
    if (!status.running) return;
    const timer = setInterval(() => {
      setNow(Date.now());
      void invoke<MobileStatus>("mobile_status").then(onStatus);
    }, 2000);
    return () => clearInterval(timer);
  }, [status.running, onStatus]);

  const qrSrc = useMemo(
    () =>
      status.tunnel.qrSvg
        ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(status.tunnel.qrSvg)}`
        : null,
    [status.tunnel.qrSvg],
  );

  const run = async (command: "mobile_start" | "mobile_stop") => {
    setBusy(true);
    setError(null);
    try {
      if (command === "mobile_stop") {
        await invoke(command);
      } else {
        onStatus(await invoke<MobileStatus>(command));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const restart = async () => {
    await run("mobile_stop");
    await run("mobile_start");
  };

  const { tunnel } = status;
  const connected = phoneConnected(status, now);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start justify-between gap-2">
        <div className="flex flex-col gap-0.5">
          <span className="font-medium text-foreground">Phone access</span>
          <span className="text-muted-foreground text-xs">
            {!status.running
              ? "Watch and control terminals from your phone"
              : connected
                ? "Phone connected"
                : tunnel.state === "ready"
                  ? "Scan the code with your phone camera"
                  : "Preparing secure link"}
          </span>
        </div>
        {status.running && (
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => void run("mobile_stop")}
          >
            Turn off
          </Button>
        )}
      </div>

      {!status.running && (
        <>
          <p className="text-muted-foreground text-xs leading-relaxed">
            Creates a private link through a Cloudflare tunnel that works from
            any network. Anyone with the link can type into your terminals, so
            share it only with your own devices. Private tabs stay hidden.
          </p>
          <Button disabled={busy} onClick={() => void run("mobile_start")}>
            {busy ? <Spinner /> : null}
            Turn on
          </Button>
        </>
      )}

      {status.running && tunnel.state === "starting" && (
        <div className="flex h-40 items-center justify-center gap-2 text-muted-foreground text-xs">
          <Spinner /> Creating tunnel
        </div>
      )}

      {status.running && tunnel.state === "ready" && qrSrc && tunnel.link && (
        <>
          <div className="flex justify-center rounded-2xl bg-white p-2">
            <img
              src={qrSrc}
              alt="QR code with the phone access link"
              className="size-52"
              draggable={false}
            />
          </div>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              className="flex-1"
              onClick={() => copy("link", tunnel.link ?? "")}
            >
              {copied === "link" ? "Copied" : "Copy link"}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => void restart()}
              title="Revoke this link and create a new one"
            >
              New link
            </Button>
          </div>
          <p className="truncate text-muted-foreground text-xs">
            {tunnel.publicUrl}
          </p>
        </>
      )}

      {status.running && tunnel.state === "missing" && (
        <div className="flex flex-col gap-2 text-xs">
          <p className="text-muted-foreground leading-relaxed">
            The tunnel needs cloudflared. Install it, then try again.
          </p>
          <button
            type="button"
            onClick={() => copy("install", INSTALL_COMMAND)}
            className="rounded-lg bg-muted px-2.5 py-2 text-left font-mono text-foreground hover:bg-accent"
          >
            {copied === "install" ? "Copied" : INSTALL_COMMAND}
          </button>
          <Button size="sm" disabled={busy} onClick={() => void restart()}>
            Try again
          </Button>
        </div>
      )}

      {status.running && tunnel.state === "failed" && (
        <div className="flex flex-col gap-2 text-xs">
          <p className="break-words text-destructive leading-relaxed">
            {tunnel.error ?? "The tunnel stopped."}
          </p>
          <Button size="sm" disabled={busy} onClick={() => void restart()}>
            Try again
          </Button>
        </div>
      )}

      {error && <p className="text-destructive text-xs">{error}</p>}
    </div>
  );
}
