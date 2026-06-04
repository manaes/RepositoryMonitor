export type DesktopPlatform = "macos" | "windows" | "linux";

export function desktopPlatform(): DesktopPlatform {
  const platform = navigator.platform.toLowerCase();
  const ua = navigator.userAgent.toLowerCase();
  if (platform.includes("win") || ua.includes("windows")) return "windows";
  if (platform.includes("mac") || ua.includes("mac os")) return "macos";
  return "linux";
}

export function fileManagerName(platform = desktopPlatform()): string {
  if (platform === "windows") return "Explorer";
  if (platform === "macos") return "Finder";
  return "File Manager";
}
