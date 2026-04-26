export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const nativeOnlyMessage =
  "Browser preview cannot access native file dialogs or local project paths. Run the app with npm run tauri dev to use this feature.";
