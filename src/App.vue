<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
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
const themeOverrides = {
  common: {
    primaryColor: "#8fa7bd",
    primaryColorHover: "#a4c4d7",
    primaryColorPressed: "#6f8598",
    primaryColorSuppl: "#b6c8d6",
    successColor: "#7fb5a2",
    warningColor: "#ffb02e",
    errorColor: "#ff4568",
    infoColor: "#a4c4d7",
    textColorBase: "#edf4fb",
    bodyColor: "#09111a",
    modalColor: "rgba(16, 26, 36, 0.74)",
    cardColor: "rgba(16, 26, 36, 0.74)",
    popoverColor: "rgba(16, 26, 36, 0.84)",
    borderColor: "rgba(112, 133, 151, 0.18)",
  },
  Button: {
    borderRadiusMedium: "8px",
    textColor: "#c8d5df",
    textColorHover: "#edf4fb",
    textColorPressed: "#edf4fb",
    textColorFocus: "#edf4fb",
    textColorDisabled: "rgba(200, 213, 223, 0.42)",
    textColorPrimary: "#edf4fb",
    textColorHoverPrimary: "#edf4fb",
    textColorPressedPrimary: "#edf4fb",
    textColorFocusPrimary: "#edf4fb",
    textColorDisabledPrimary: "rgba(237, 244, 251, 0.46)",
    color: "rgba(255, 255, 255, 0.06)",
    colorHover: "rgba(112, 133, 151, 0.12)",
    colorPressed: "rgba(112, 133, 151, 0.18)",
    colorFocus: "rgba(112, 133, 151, 0.12)",
    colorDisabled: "rgba(255, 255, 255, 0.035)",
    colorPrimary: "linear-gradient(135deg, #8fa7bd, #a4c4d7)",
    colorHoverPrimary: "linear-gradient(135deg, #a5b7c7, #c8d7e2)",
    colorPressedPrimary: "linear-gradient(135deg, #62788c, #879bac)",
    colorFocusPrimary: "linear-gradient(135deg, #a5b7c7, #c8d7e2)",
    colorDisabledPrimary: "rgba(143, 167, 189, 0.18)",
    border: "1px solid rgba(112, 133, 151, 0.22)",
    borderHover: "1px solid #a4c4d7",
    borderFocus: "1px solid #a4c4d7",
    borderPressed: "1px solid #8fa7bd",
    borderDisabled: "1px solid rgba(112, 133, 151, 0.16)",
    borderPrimary: "1px solid rgba(164, 196, 215, 0.62)",
    borderHoverPrimary: "1px solid rgba(210, 230, 241, 0.86)",
    borderPressedPrimary: "1px solid rgba(164, 196, 215, 0.68)",
    borderFocusPrimary: "1px solid rgba(210, 230, 241, 0.86)",
    borderDisabledPrimary: "1px solid rgba(143, 167, 189, 0.24)",
  },
  Input: {
    color: "rgba(255, 255, 255, 0.08)",
    colorFocus: "rgba(112, 133, 151, 0.12)",
    border: "1px solid rgba(112, 133, 151, 0.16)",
    borderHover: "1px solid #8fa7bd",
    borderFocus: "1px solid #8fa7bd",
    boxShadowFocus: "0 0 0 2px rgba(112, 133, 151, 0.18)",
    placeholderColor: "rgba(200, 213, 223, 0.46)",
  },
  InputNumber: {
    peers: {
      Input: {
        color: "rgba(255, 255, 255, 0.08)",
        colorFocus: "rgba(112, 133, 151, 0.12)",
      },
    },
  },
  Select: {
    peers: {
      InternalSelection: {
        color: "rgba(255, 255, 255, 0.08)",
        colorActive: "rgba(112, 133, 151, 0.12)",
        border: "1px solid rgba(112, 133, 151, 0.16)",
        borderHover: "1px solid #8fa7bd",
        borderActive: "1px solid #8fa7bd",
        boxShadowActive: "0 0 0 2px rgba(112, 133, 151, 0.18)",
      },
      InternalSelectMenu: {
        color: "rgba(16, 26, 36, 0.92)",
        optionColorPending: "rgba(112, 133, 151, 0.14)",
        optionColorActive: "rgba(112, 133, 151, 0.2)",
      },
    },
  },
};

const tabs = [
  { id: "projects" as const, labelKey: "nav.projects", icon: "P" },
  { id: "logs" as const, labelKey: "nav.logs", icon: "L" },
  { id: "settings" as const, labelKey: "nav.settings", icon: "S" },
];

onMounted(async () => {
  store.setupEventListener();
  await store.loadConfig();
  await store.loadProjects();
  await store.refreshStatuses();
  void store.refreshGitStatuses();
  if (store.config.auto_check_updates) {
    await store.checkForAppUpdate({ silent: true });
  }
});

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
  <NConfigProvider :theme="darkTheme" :theme-overrides="themeOverrides">
  <div class="app-layout">
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
            class="update-action-primary"
            type="primary"
            @click="installUpdate"
          >
            {{ t("update.install") }}
          </NButton>
          <NButton
            v-if="store.appUpdateStatus === 'installed'"
            class="update-action-primary"
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
    radial-gradient(ellipse 72% 56% at 0% 0%, rgba(22, 158, 196, 0.364), transparent 66%),
    radial-gradient(ellipse 62% 48% at 98% 0%, rgba(29, 51, 82, 0.34), transparent 70%),
    radial-gradient(ellipse 72% 42% at 52% 10%, rgba(23, 40, 57, 0.3), transparent 76%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.025), rgba(255, 255, 255, 0) 38%),
    var(--color-bg);
}

.app-layout::before {
  position: absolute;
  inset: 0;
  content: "";
  pointer-events: none;
  background:
    linear-gradient(90deg, rgba(68, 95, 92, 0.1), transparent 24%, transparent 78%, rgba(62, 82, 117, 0.14)),
    linear-gradient(180deg, transparent 0%, rgba(0, 0, 0, 0.36) 74%, rgba(0, 0, 0, 0.7) 100%);
}

.sidebar {
  position: relative;
  z-index: 1;
  width: 224px;
  flex-shrink: 0;
  background: rgba(8, 14, 21, 0.72);
  border-right: 1px solid rgba(112, 133, 151, 0.15);
  display: flex;
  flex-direction: column;
  padding: 0;
  box-shadow: 20px 0 70px rgba(0, 0, 0, 0.24);
  backdrop-filter: blur(26px) saturate(150%);
}

.sidebar-header {
  padding: 22px 18px 18px;
  border-bottom: 1px solid rgba(112, 133, 151, 0.13);
}

.brand-mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  margin-bottom: 12px;
  border: 1px solid rgba(112, 133, 151, 0.22);
  border-radius: 12px;
  background:
    linear-gradient(145deg, rgba(164, 196, 215, 0.18), rgba(112, 133, 151, 0.24));
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
  box-shadow: 0 0 36px rgba(112, 133, 151, 0.26);
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
  border-color: rgba(112, 133, 151, 0.22);
  background: rgba(112, 133, 151, 0.1);
  color: var(--color-text);
}

.nav-item.active {
  border-color: rgba(112, 133, 151, 0.46);
  background: linear-gradient(90deg, rgba(112, 133, 151, 0.24), rgba(164, 196, 215, 0.1));
  color: var(--color-text);
  box-shadow: inset 3px 0 0 var(--color-primary), 0 0 34px rgba(112, 133, 151, 0.2);
}

.nav-icon {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(112, 133, 151, 0.18);
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
  border-top: 1px solid rgba(112, 133, 151, 0.13);
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
  box-shadow: 0 0 18px rgba(112, 133, 151, 0.55);
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

:deep(.update-action-primary.n-button) {
  --n-text-color: #edf4fb !important;
  --n-text-color-hover: #edf4fb !important;
  --n-text-color-pressed: #edf4fb !important;
  --n-text-color-focus: #edf4fb !important;
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
