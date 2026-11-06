export function browserLaunchOptions(args?: string[]): { channel: "chrome" | "chromium"; args?: string[] } {
  return {
    channel: process.env.THEME_PARK_PLAYWRIGHT_DOCKER === "1" ? "chromium" : "chrome",
    ...(args ? { args } : {}),
  };
}
