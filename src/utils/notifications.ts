import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

let permissionReady: Promise<boolean> | null = null;

async function ensureNotificationPermission() {
  if (!permissionReady) {
    permissionReady = (async () => {
      try {
        let granted = await isPermissionGranted();
        if (!granted) {
          granted = (await requestPermission()) === "granted";
        }
        return granted;
      } catch (error) {
        console.warn("Notification permission failed:", error);
        return false;
      }
    })();
  }

  return permissionReady;
}

export async function notifyUser(title: string, body?: string) {
  const granted = await ensureNotificationPermission();
  if (!granted) return;

  sendNotification({
    title,
    body,
    group: "prostation",
    autoCancel: true,
  });
}
