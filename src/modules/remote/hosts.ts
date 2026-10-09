import { invoke } from "@tauri-apps/api/core";

export type SshHost = {
  alias: string;
  hostname: string | null;
  user: string | null;
  port: number | null;
};

const ALIAS_RE = /^[A-Za-z0-9._@:%[\]_][A-Za-z0-9._@:%[\]_-]{0,254}$/;

export function isValidSshAlias(alias: string): boolean {
  return ALIAS_RE.test(alias);
}

export function buildSshCommand(alias: string): string | null {
  if (!isValidSshAlias(alias)) return null;
  return `ssh -- '${alias}'\r`;
}

export function sshSpaceInput(
  alias: string,
): { name: string; env: { kind: "ssh"; host: string } } | null {
  if (!isValidSshAlias(alias)) return null;
  return { name: alias, env: { kind: "ssh", host: alias } };
}

export function sshHostDetail(host: SshHost): string {
  const target = host.hostname ?? host.alias;
  const withUser = host.user ? `${host.user}@${target}` : target;
  return host.port ? `${withUser}:${host.port}` : withUser;
}

export function listSshHosts(): Promise<SshHost[]> {
  return invoke<SshHost[]>("ssh_list_hosts");
}
