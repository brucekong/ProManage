<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { NButton, NConfigProvider, NModal, darkTheme } from "naive-ui";
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

const updateModalTitle = computed(() => {
  if (store.appUpdateStatus === "available" && store.availableAppUpdate) {
    return `ProStation ${store.availableAppUpdate.version}`;
  }
  return t("update.updater");
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
  <NConfigProvider :theme="darkTheme">
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
      :title="t('update.kicker')"
      style="width: 420px; max-width: 90vw;"
      :bordered="false"
      :closable="store.appUpdateStatus !== 'checking' && store.appUpdateStatus !== 'installing'"
      :mask-closable="store.appUpdateStatus !== 'checking' && store.appUpdateStatus !== 'installing'"
    >
      <div class="update-dialog" :class="store.appUpdateStatus">
        <span class="update-dialog-mark"></span>
        <div class="update-dialog-copy">
          <strong>{{ updateModalTitle }}</strong>
          <span>{{ store.appUpdateMessage || t("update.checking") }}</span>
        </div>
      </div>
      <template #footer>
        <div class="update-dialog-actions">
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
        </div>
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
    radial-gradient(ellipse 70% 42% at 50% 3%, rgba(64, 148, 216, 0.22), transparent 64%),
    radial-gradient(ellipse 52% 36% at 72% 20%, rgba(36, 152, 161, 0.16), transparent 68%),
    radial-gradient(ellipse 42% 34% at 22% 18%, rgba(83, 116, 192, 0.1), transparent 72%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.025), rgba(255, 255, 255, 0)),
    var(--color-bg);
}

.app-layout::before {
  position: absolute;
  inset: 0;
  content: "";
  pointer-events: none;
  background:
    linear-gradient(90deg, rgba(255, 255, 255, 0.035), transparent 16%, transparent 84%, rgba(255, 255, 255, 0.02)),
    linear-gradient(180deg, transparent 0%, rgba(0, 0, 0, 0.34) 78%, rgba(0, 0, 0, 0.62) 100%);
}

.sidebar {
  position: relative;
  z-index: 1;
  width: 224px;
  flex-shrink: 0;
  background: rgba(12, 12, 15, 0.64);
  border-right: 1px solid rgba(190, 224, 255, 0.1);
  display: flex;
  flex-direction: column;
  padding: 0;
  box-shadow: 20px 0 70px rgba(0, 0, 0, 0.24);
  backdrop-filter: blur(26px) saturate(120%);
}

.sidebar-header {
  padding: 22px 18px 18px;
  border-bottom: 1px solid rgba(190, 224, 255, 0.09);
}

.brand-mark {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  margin-bottom: 12px;
  border: 1px solid rgba(190, 224, 255, 0.16);
  border-radius: 12px;
  background:
    linear-gradient(145deg, rgba(190, 224, 255, 0.12), rgba(61, 159, 230, 0.16));
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
  box-shadow: 0 0 36px rgba(82, 169, 235, 0.18);
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
  border-color: rgba(190, 224, 255, 0.14);
  background: rgba(190, 224, 255, 0.06);
  color: var(--color-text);
}

.nav-item.active {
  border-color: rgba(105, 186, 245, 0.28);
  background: linear-gradient(90deg, rgba(105, 186, 245, 0.14), rgba(134, 217, 233, 0.06));
  color: var(--color-text);
  box-shadow: inset 3px 0 0 var(--color-primary), 0 0 34px rgba(82, 169, 235, 0.12);
}

.nav-icon {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(190, 224, 255, 0.12);
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
  border-top: 1px solid rgba(190, 224, 255, 0.09);
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
  box-shadow: 0 0 18px rgba(105, 186, 245, 0.5);
}

.update-dialog.available .update-dialog-mark,
.update-dialog.installed .update-dialog-mark,
.update-dialog.up-to-date .update-dialog-mark {
  background: var(--color-green);
  box-shadow: 0 0 18px rgba(124, 226, 188, 0.45);
}

.update-dialog.error .update-dialog-mark,
.update-dialog.disabled .update-dialog-mark {
  background: var(--color-red);
  box-shadow: 0 0 18px rgba(255, 109, 130, 0.35);
}

.update-dialog.checking .update-dialog-mark,
.update-dialog.installing .update-dialog-mark {
  animation: updatePulse 1s ease-in-out infinite;
}

.update-dialog-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.update-dialog-copy strong {
  color: var(--color-text);
  font-size: 16px;
}

.update-dialog-copy span {
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.update-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
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
  overflow: hidden;
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
