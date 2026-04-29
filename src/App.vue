<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { NButton, NConfigProvider, NModal, NSpace, darkTheme } from "naive-ui";
import { useProjectStore } from "./stores/project";
import ProjectList from "./views/ProjectList.vue";
import LogPanel from "./views/LogPanel.vue";
import SettingsView from "./views/SettingsView.vue";
import TerminalPanel from "./components/TerminalPanel.vue";
import { useI18n } from "./i18n";
import packageJson from "../package.json";

const store = useProjectStore();
const { t } = useI18n();
const appVersion = packageJson.version;
const updateModalVisible = ref(false);
const systemPrefersDark = ref(true);

const darkThemeOverrides = {
  common: {
    primaryColor: "#35d1df",
    primaryColorHover: "#b8f2f6",
    primaryColorPressed: "#1497a3",
    primaryColorSuppl: "#7fe9f1",
    successColor: "#76c893",
    warningColor: "#ffc131",
    errorColor: "#ff4568",
    infoColor: "#35d1df",
    textColorBase: "#f4f2ea",
    bodyColor: "#111513",
    modalColor: "rgba(20, 24, 22, 0.88)",
    cardColor: "rgba(20, 24, 22, 0.86)",
    popoverColor: "rgba(20, 24, 22, 0.94)",
    borderColor: "rgba(198, 220, 215, 0.16)",
  },
  Button: {
    borderRadiusMedium: "8px",
    textColorPrimary: "#071315",
    textColorHover: "#b8f2f6",
    color: "rgba(255, 244, 206, 0.055)",
    colorHover: "rgba(53, 209, 223, 0.1)",
    colorPressed: "rgba(53, 209, 223, 0.16)",
    colorPrimary: "linear-gradient(135deg, #35d1df, #ffc131)",
    colorHoverPrimary: "linear-gradient(135deg, #7fe9f1, #ffd56f)",
    colorPressedPrimary: "linear-gradient(135deg, #1497a3, #c98b15)",
    borderHover: "1px solid #35d1df",
    borderFocus: "1px solid #35d1df",
  },
  Input: {
    color: "rgba(255, 244, 206, 0.06)",
    colorFocus: "rgba(53, 209, 223, 0.08)",
    border: "1px solid rgba(198, 220, 215, 0.16)",
    borderHover: "1px solid #35d1df",
    borderFocus: "1px solid #35d1df",
    boxShadowFocus: "0 0 0 2px rgba(53, 209, 223, 0.16)",
    placeholderColor: "rgba(211, 208, 198, 0.48)",
  },
  InputNumber: {
    peers: {
      Input: {
        color: "rgba(255, 244, 206, 0.06)",
        colorFocus: "rgba(53, 209, 223, 0.08)",
      },
    },
  },
  Select: {
    peers: {
      InternalSelection: {
        color: "rgba(255, 244, 206, 0.06)",
        colorActive: "rgba(53, 209, 223, 0.08)",
        border: "1px solid rgba(198, 220, 215, 0.16)",
        borderHover: "1px solid #35d1df",
        borderActive: "1px solid #35d1df",
        boxShadowActive: "0 0 0 2px rgba(53, 209, 223, 0.16)",
      },
      InternalSelectMenu: {
        color: "rgba(20, 24, 22, 0.96)",
        optionColorPending: "rgba(53, 209, 223, 0.1)",
        optionColorActive: "rgba(255, 193, 49, 0.11)",
      },
    },
  },
};

const lightThemeOverrides = {
  common: {
    primaryColor: "#159eab",
    primaryColorHover: "#20b8c5",
    primaryColorPressed: "#0e7882",
    primaryColorSuppl: "#6bdde6",
    successColor: "#4b9270",
    warningColor: "#c88a16",
    errorColor: "#d83d5b",
    infoColor: "#159eab",
    textColorBase: "#172320",
    bodyColor: "#f5f1e7",
    modalColor: "rgba(255, 252, 242, 0.96)",
    cardColor: "rgba(255, 252, 242, 0.92)",
    popoverColor: "rgba(255, 252, 242, 0.98)",
    borderColor: "rgba(38, 104, 105, 0.18)",
  },
  Button: {
    borderRadiusMedium: "8px",
    textColorPrimary: "#071315",
    textColorHover: "#159eab",
    color: "rgba(255, 252, 242, 0.74)",
    colorHover: "rgba(32, 169, 180, 0.1)",
    colorPressed: "rgba(32, 169, 180, 0.16)",
    colorPrimary: "linear-gradient(135deg, #35d1df, #ffc131)",
    colorHoverPrimary: "linear-gradient(135deg, #52dae5, #ffd36a)",
    colorPressedPrimary: "linear-gradient(135deg, #159eab, #c88a16)",
    borderHover: "1px solid #159eab",
    borderFocus: "1px solid #159eab",
  },
  Input: {
    color: "rgba(255, 252, 242, 0.74)",
    colorFocus: "rgba(255, 252, 242, 0.94)",
    border: "1px solid rgba(38, 104, 105, 0.2)",
    borderHover: "1px solid #159eab",
    borderFocus: "1px solid #159eab",
    boxShadowFocus: "0 0 0 2px rgba(32, 169, 180, 0.15)",
    placeholderColor: "rgba(65, 86, 80, 0.52)",
  },
  InputNumber: {
    peers: {
      Input: {
        color: "rgba(255, 252, 242, 0.74)",
        colorFocus: "rgba(255, 252, 242, 0.94)",
      },
    },
  },
  Select: {
    peers: {
      InternalSelection: {
        color: "rgba(255, 252, 242, 0.74)",
        colorActive: "rgba(255, 252, 242, 0.94)",
        border: "1px solid rgba(38, 104, 105, 0.2)",
        borderHover: "1px solid #159eab",
        borderActive: "1px solid #159eab",
        boxShadowActive: "0 0 0 2px rgba(32, 169, 180, 0.15)",
      },
      InternalSelectMenu: {
        color: "rgba(255, 252, 242, 0.98)",
        optionColorPending: "rgba(32, 169, 180, 0.1)",
        optionColorActive: "rgba(255, 193, 49, 0.13)",
      },
    },
  },
};

const resolvedTheme = computed<"light" | "dark">(() => {
  if (store.config.theme === "light") return "light";
  if (store.config.theme === "dark") return "dark";
  return systemPrefersDark.value ? "dark" : "light";
});
const naiveTheme = computed(() => (resolvedTheme.value === "dark" ? darkTheme : null));
const themeOverrides = computed(() =>
  resolvedTheme.value === "dark" ? darkThemeOverrides : lightThemeOverrides
);

const tabs = [
  { id: "projects" as const, labelKey: "nav.projects", icon: "P" },
  { id: "logs" as const, labelKey: "nav.logs", icon: "L" },
  { id: "settings" as const, labelKey: "nav.settings", icon: "S" },
];

let themeMediaQuery: MediaQueryList | null = null;
let removeThemeListener: (() => void) | null = null;

onMounted(async () => {
  if (typeof window !== "undefined") {
    themeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    systemPrefersDark.value = themeMediaQuery.matches;
    const handleThemeChange = (event: MediaQueryListEvent) => {
      systemPrefersDark.value = event.matches;
    };
    themeMediaQuery.addEventListener("change", handleThemeChange);
    removeThemeListener = () => themeMediaQuery?.removeEventListener("change", handleThemeChange);
  }

  store.setupEventListener();
  await store.loadConfig();
  await store.loadProjects();
  await store.refreshStatuses();
  void store.refreshGitStatuses();
  if (store.config.auto_check_updates) {
    await store.checkForAppUpdate({ silent: true });
  }
});

onUnmounted(() => {
  removeThemeListener?.();
});

watch(
  resolvedTheme,
  (theme) => {
    document.documentElement.dataset.theme = theme;
  },
  { immediate: true }
);

const showUpdateActions = computed(() =>
  ["disabled", "checking", "available", "installing", "installed", "up-to-date", "error"].includes(store.appUpdateStatus)
);

watch(
  () => store.appUpdateStatus,
  (status) => {
    if (showUpdateActions.value && status !== "idle" && store.appUpdateMessage) {
      updateModalVisible.value = true;
    }
  }
);

async function checkUpdatesFromVersion() {
  updateModalVisible.value = true;
  await store.checkForAppUpdate({
    messages: {
      disabled: t("update.disabled"),
      checking: t("update.checking"),
      ready: (version) => t("update.ready", { version }),
      latest: t("update.latest"),
    },
  });
}

async function installUpdate() {
  updateModalVisible.value = true;
  await store.installAvailableAppUpdate();
}

async function relaunchApp() {
  await store.relaunchApp();
}
</script>

<template>
  <NConfigProvider :theme="naiveTheme" :theme-overrides="themeOverrides">
  <div class="app-layout" :data-theme="resolvedTheme">
    <aside class="sidebar">
      <div class="sidebar-header">
        <h1 class="app-title">ProStation</h1>
        <p class="app-subtitle">{{ t("app.subtitle") }}</p>
      </div>

      <nav class="sidebar-nav">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="nav-item"
          :class="{ active: store.activeTab === tab.id }"
          @click="store.activeTab = tab.id"
        >
          <span class="nav-icon">{{ tab.icon }}</span>
          <span class="nav-label">{{ t(tab.labelKey) }}</span>
          <span
            v-if="tab.id === 'projects' && store.runningCount > 0"
            class="nav-badge"
          >
            {{ store.runningCount }}
          </span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <div class="system-pulse">
          <span class="pulse-dot"></span>
          <span>{{ t("status.online", { count: store.runningCount }) }}</span>
        </div>
        <button
          class="version"
          type="button"
          :title="t('update.check')"
          :aria-label="t('update.check')"
          @click="checkUpdatesFromVersion"
        >
          v{{ appVersion }}
        </button>
      </div>
    </aside>

    <main class="main-content">
      <section class="workspace-panel">
        <ProjectList v-if="store.activeTab === 'projects'" />
        <LogPanel v-else-if="store.activeTab === 'logs'" />
        <SettingsView v-else-if="store.activeTab === 'settings'" />
      </section>
      <TerminalPanel />
    </main>

    <NModal
      v-model:show="updateModalVisible"
      preset="card"
      class="glass-modal"
      :title="t('update.kicker')"
      style="width: 420px; max-width: 90vw;"
      :bordered="false"
      :closable="store.appUpdateStatus !== 'checking' && store.appUpdateStatus !== 'installing'"
      :mask-closable="store.appUpdateStatus !== 'checking' && store.appUpdateStatus !== 'installing'"
    >
      <div class="update-dialog" :class="store.appUpdateStatus">
        <span class="update-dialog-mark"></span>
        <div class="update-dialog-copy">
          <span>{{ store.appUpdateMessage || t("update.checking") }}</span>
        </div>
      </div>
      <template #footer>
        <NSpace justify="end">
          <NButton
            v-if="store.appUpdateStatus === 'available'"
            type="primary"
            @click="installUpdate"
          >
            {{ t("update.install") }}
          </NButton>
          <NButton
            v-if="store.appUpdateStatus === 'installed'"
            type="primary"
            @click="relaunchApp"
          >
            {{ t("common.restart") }}
          </NButton>
          <NButton
            v-if="store.appUpdateStatus !== 'checking' && store.appUpdateStatus !== 'installing'"
            @click="updateModalVisible = false"
          >
            {{ t("update.close") }}
          </NButton>
        </NSpace>
      </template>
    </NModal>
  </div>
  </NConfigProvider>
</template>

<style scoped>
.app-layout {
  position: relative;
  display: flex;
  height: 100vh;
  overflow: hidden;
  background:
    radial-gradient(ellipse 72% 56% at 0% 0%, var(--app-glow-start), transparent 66%),
    radial-gradient(ellipse 62% 48% at 98% 0%, var(--app-glow-end), transparent 70%),
    radial-gradient(ellipse 72% 42% at 52% 10%, var(--app-glow-center), transparent 76%),
    linear-gradient(var(--app-grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--app-grid-line) 1px, transparent 1px),
    linear-gradient(180deg, var(--app-sheen), transparent 38%),
    var(--color-bg);
  background-size: auto, auto, auto, 24px 24px, 24px 24px, auto, auto;
}

.app-layout::before {
  position: absolute;
  inset: 0;
  content: "";
  pointer-events: none;
  background:
    linear-gradient(90deg, var(--app-edge-start), transparent 24%, transparent 78%, var(--app-edge-end)),
    linear-gradient(180deg, transparent 0%, var(--app-bottom-fade) 100%);
}

.sidebar {
  position: relative;
  z-index: 1;
  width: 224px;
  flex-shrink: 0;
  background: var(--sidebar-bg);
  border-right: 1px solid var(--sidebar-border);
  display: flex;
  flex-direction: column;
  padding: 0;
  box-shadow: 20px 0 70px var(--sidebar-shadow);
  backdrop-filter: blur(26px) saturate(150%);
}

.sidebar-header {
  padding: 22px 18px 18px;
  border-bottom: 1px solid var(--sidebar-border);
}

.brand-mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  margin-bottom: 12px;
  border: 1px solid rgba(53, 209, 223, 0.24);
  border-radius: 12px;
  background:
    linear-gradient(145deg, rgba(53, 209, 223, 0.2), rgba(255, 193, 49, 0.16));
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
  box-shadow: 0 0 36px rgba(53, 209, 223, 0.18);
}

.app-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--color-text);
  margin: 0;
}

.app-subtitle {
  font-size: 11px;
  color: var(--color-muted);
  margin: 3px 0 0;
}

.sidebar-nav {
  flex: 1;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 42px;
  padding: 10px 11px;
  border-radius: 8px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s;
  text-align: left;
  width: 100%;
}

.nav-item:hover {
  border-color: rgba(53, 209, 223, 0.24);
  background: rgba(53, 209, 223, 0.08);
  color: var(--color-text);
}

.nav-item.active {
  border-color: rgba(53, 209, 223, 0.44);
  background: linear-gradient(90deg, rgba(53, 209, 223, 0.18), rgba(255, 193, 49, 0.08));
  color: var(--color-text);
  box-shadow: inset 3px 0 0 var(--color-primary), 0 0 34px rgba(53, 209, 223, 0.15);
}

.nav-icon {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(53, 209, 223, 0.18);
  border-radius: 8px;
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 800;
  text-align: center;
}

.nav-badge {
  margin-left: auto;
  background: rgba(255, 255, 255, 0.25);
  color: white;
  font-size: 11px;
  font-weight: 600;
  padding: 1px 7px;
  border-radius: 10px;
}

.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px 16px;
  border-top: 1px solid var(--sidebar-border);
}

.system-pulse {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-green);
  box-shadow: 0 0 16px var(--color-green);
}

.version {
  width: fit-content;
  padding: 0;
  border: 0;
  background: transparent;
  font-size: 11px;
  color: var(--color-muted);
  cursor: pointer;
  text-align: left;
}

.version:hover,
.version:focus-visible {
  color: var(--color-primary);
  text-decoration: underline;
  outline: none;
}

.update-dialog {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 4px 0 2px;
}

.update-dialog-mark {
  width: 12px;
  height: 12px;
  flex: 0 0 auto;
  margin-top: 4px;
  border-radius: 999px;
  background: var(--color-primary);
  box-shadow: 0 0 18px rgba(53, 209, 223, 0.45);
}

.update-dialog.available .update-dialog-mark,
.update-dialog.installed .update-dialog-mark,
.update-dialog.up-to-date .update-dialog-mark {
  background: var(--color-green);
  box-shadow: 0 0 18px rgba(39, 242, 154, 0.48);
}

.update-dialog.error .update-dialog-mark,
.update-dialog.disabled .update-dialog-mark {
  background: var(--color-red);
  box-shadow: 0 0 18px rgba(255, 69, 104, 0.42);
}

.update-dialog.checking .update-dialog-mark,
.update-dialog.installing .update-dialog-mark {
  animation: updatePulse 1s ease-in-out infinite;
}

.update-dialog-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.update-dialog-copy span {
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.5;
}

@keyframes updatePulse {
  0%,
  100% {
    opacity: 0.55;
    transform: scale(0.86);
  }

  50% {
    opacity: 1;
    transform: scale(1.08);
  }
}

.main-content {
  position: relative;
  z-index: 1;
  flex: 1;
  min-width: 0;
  display: flex;
  overflow: hidden;
}

.workspace-panel {
  min-width: 0;
  flex: 1;
  padding: 18px;
  overflow-y: auto;
}

@media (max-width: 980px) {
  .sidebar {
    width: 78px;
  }

  .app-title,
  .app-subtitle,
  .nav-label,
  .sidebar-footer {
    display: none;
  }

  .sidebar-header {
    padding: 16px;
  }
}
</style>
