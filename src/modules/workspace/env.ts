import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { setLastWslDistro } from "@/modules/settings/store";

export type WorkspaceEnv =
  | { kind: "local" }
  | { kind: "wsl"; distro: string }
  | { kind: "ssh"; host: string };

export type WslDistro = {
  name: string;
  default: boolean;
  running: boolean;
};

type State = {
  env: WorkspaceEnv;
  distros: WslDistro[];
  loading: boolean;
  error: string | null;
  setEnv: (env: WorkspaceEnv) => void;
  refreshDistros: () => Promise<WslDistro[]>;
};

export const LOCAL_WORKSPACE: WorkspaceEnv = { kind: "local" };

export const useWorkspaceEnvStore = create<State>((set) => ({
  env: LOCAL_WORKSPACE,
  distros: [],
  loading: false,
  error: null,
  setEnv: (env) => {
    set({ env });
    if (env.kind === "wsl") void setLastWslDistro(env.distro);
  },
  refreshDistros: async () => {
    set({ loading: true, error: null });
    try {
      const distros = await invoke<WslDistro[]>("wsl_list_distros");
      set({ distros, loading: false });
      return distros;
    } catch (e) {
      set({ distros: [], loading: false, error: String(e) });
      return [];
    }
  },
}));

export function currentWorkspaceEnv(): WorkspaceEnv {
  return useWorkspaceEnvStore.getState().env;
}

export function assertNever(value: never): never {
  throw new Error(`Unhandled workspace env: ${JSON.stringify(value)}`);
}

export function workspaceScopeKey(env: WorkspaceEnv): string {
  switch (env.kind) {
    case "local":
      return "local";
    case "wsl":
      return `wsl:${env.distro}`;
    case "ssh":
      return `ssh:${env.host}`;
    default:
      return assertNever(env);
  }
}

export function parseWorkspaceScopeKey(key: string): WorkspaceEnv {
  if (key.startsWith("wsl:")) {
    return { kind: "wsl", distro: key.slice("wsl:".length) };
  }
  if (key.startsWith("ssh:")) {
    return { kind: "ssh", host: key.slice("ssh:".length) };
  }
  return LOCAL_WORKSPACE;
}

export function sameWorkspaceEnv(a: WorkspaceEnv, b: WorkspaceEnv): boolean {
  return workspaceScopeKey(a) === workspaceScopeKey(b);
}

/** Events without an env tag are treated as matching, for older emitters. */
export function fsEventInEnv(
  eventEnv: string | undefined,
  env: WorkspaceEnv,
): boolean {
  return eventEnv === undefined || eventEnv === workspaceScopeKey(env);
}

export type EnvCaps = {
  watch: boolean;
  gitDecorations: boolean;
  reveal: boolean;
  mediaPreview: boolean;
  htmlPreview: boolean;
  lsp: boolean;
  git: boolean;
  formatOnSave: boolean;
  aiShell: boolean;
  aiWrite: boolean;
};

const LOCAL_CAPS: EnvCaps = {
  watch: true,
  gitDecorations: true,
  reveal: true,
  mediaPreview: true,
  htmlPreview: true,
  lsp: true,
  git: true,
  formatOnSave: true,
  aiShell: true,
  aiWrite: true,
};

const WSL_CAPS: EnvCaps = {
  ...LOCAL_CAPS,
  reveal: false,
  htmlPreview: false,
  lsp: false,
};

const SSH_CAPS: EnvCaps = {
  watch: false,
  gitDecorations: false,
  reveal: false,
  mediaPreview: false,
  htmlPreview: false,
  lsp: false,
  git: false,
  formatOnSave: false,
  aiShell: false,
  aiWrite: false,
};

export function envCaps(env: WorkspaceEnv): EnvCaps {
  switch (env.kind) {
    case "local":
      return LOCAL_CAPS;
    case "wsl":
      return WSL_CAPS;
    case "ssh":
      return SSH_CAPS;
    default:
      return assertNever(env);
  }
}

export function currentWorkspaceScopeKey(): string {
  return workspaceScopeKey(currentWorkspaceEnv());
}

export async function getWslHome(distro: string): Promise<string> {
  return invoke<string>("wsl_home", { distro });
}
