import { invoke } from "@tauri-apps/api/core";
import type { Connection, OptionChange, Status } from "./types";

export const getStatus = () => invoke<Status>("get_status");

/** Turning on without admin rights restarts MultiPath through the UAC prompt. */
export const setEnabled = (enabled: boolean) => invoke<Status>("set_enabled", { enabled });

export const setLinkEnabled = (id: string, enabled: boolean) =>
  invoke<Status>("set_link_enabled", { id, enabled });

export const setLinkWeight = (id: string, weight: number) =>
  invoke<Status>("set_link_weight", { id, weight });

export const setOption = (change: OptionChange) => invoke<Status>("set_option", { change });

export const listConnections = () => invoke<Connection[]>("list_connections");

/** Commands reject with the message string the Rust side returned. */
export function errorText(err: unknown): string {
  return typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
}
