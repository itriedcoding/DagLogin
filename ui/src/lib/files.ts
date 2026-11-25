import { open, save } from "@tauri-apps/plugin-dialog";

export const CONFIG_FILTER = { name: "Prism configuration", extensions: ["prism", "json"] };
export const BACKUP_FILTER = { name: "Prism backup", extensions: ["prism", "zip"] };

export async function pickSavePath(
  defaultPath: string,
  filters: { name: string; extensions: string[] }[] = [CONFIG_FILTER]
): Promise<string | null> {
  const path = await save({ defaultPath, filters });
  return typeof path === "string" ? path : null;
}

export async function pickOpenPath(
  filters: { name: string; extensions: string[] }[] = [CONFIG_FILTER]
): Promise<string | null> {
  const path = await open({ multiple: false, filters, directory: false });
  return typeof path === "string" ? path : null;
}

export function stampedName(prefix: string, extension: string): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${prefix}-${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(
    now.getHours()
  )}${pad(now.getMinutes())}.${extension}`;
}
