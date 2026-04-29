<script setup lang="ts">
import { useProjectStore } from "../stores/project";
import { computed, reactive, watch } from "vue";
import { NSelect } from "naive-ui";
import { useI18n } from "../i18n";

const store = useProjectStore();
const { t } = useI18n();

const form = reactive({ ...store.config });

const themeOptions = computed(() => [
  { label: t("settings.system"), value: "system" },
  { label: t("settings.light"), value: "light" },
  { label: t("settings.dark"), value: "dark" },
]);

const languageOptions = computed(() => [
  { label: t("settings.english"), value: "en" },
  { label: t("settings.chinese"), value: "zh" },
]);

watch(
  () => store.config,
  (nextConfig) => {
    Object.assign(form, nextConfig);
  },
  { immediate: true, deep: true }
);

async function save() {
  await store.updateAppConfig({ ...form });
}

async function applyLanguage(value: string) {
  const language = value === "zh" ? "zh" : "en";
  form.language = language;
  await store.updateAppConfig({ ...form, language });
}

async function saveAndCheckUpdates() {
  await save();
  await store.checkForAppUpdate();
}
</script>

<template>
  <div class="settings-view">
    <h2>{{ t("settings.title") }}</h2>

    <section class="setting-group">
      <h3>{{ t("settings.portRange") }}</h3>
      <div class="setting-row">
        <label>{{ t("settings.startPort") }}</label>
        <input
          v-model.number="form.port_range_start"
          type="number"
          class="setting-input"
          min="1024"
          max="65535"
        />
      </div>
      <div class="setting-row">
        <label>{{ t("settings.endPort") }}</label>
        <input
          v-model.number="form.port_range_end"
          type="number"
          class="setting-input"
          min="1024"
          max="65535"
        />
      </div>
    </section>

    <section class="setting-group">
      <h3>{{ t("settings.behavior") }}</h3>
      <div class="setting-row">
        <label>{{ t("settings.minimizeTray") }}</label>
        <input v-model="form.minimize_to_tray" type="checkbox" class="setting-checkbox" />
      </div>
      <div class="setting-row">
        <label>{{ t("settings.autoRestore") }}</label>
        <input v-model="form.auto_restore" type="checkbox" class="setting-checkbox" />
      </div>
    </section>

    <section class="setting-group">
      <h3>{{ t("settings.appearance") }}</h3>
      <div class="setting-row">
        <label>{{ t("settings.theme") }}</label>
        <NSelect
          v-model:value="form.theme"
          :options="themeOptions"
          class="setting-select"
        />
      </div>
      <div class="setting-row">
        <label>{{ t("settings.language") }}</label>
        <NSelect
          v-model:value="form.language"
          :options="languageOptions"
          class="setting-select"
          @update:value="applyLanguage"
        />
      </div>
    </section>

    <section class="setting-group">
      <h3>{{ t("settings.logging") }}</h3>
      <div class="setting-row">
        <label>{{ t("settings.retention") }}</label>
        <input
          v-model.number="form.log_retention_days"
          type="number"
          class="setting-input"
          min="1"
          max="90"
        />
      </div>
    </section>

    <section class="setting-group">
      <h3>{{ t("settings.ide") }}</h3>
      <div class="setting-row setting-row-stack">
        <label>{{ t("settings.vscode") }}</label>
        <input
          v-model="form.ide_vscode_command"
          type="text"
          class="setting-input setting-input-wide"
          placeholder="code"
        />
      </div>
      <div class="setting-row setting-row-stack">
        <label>{{ t("settings.antigravity") }}</label>
        <input
          v-model="form.ide_antigravity_command"
          type="text"
          class="setting-input setting-input-wide"
          placeholder="ag"
        />
      </div>
    </section>

    <section class="setting-group">
      <h3>{{ t("settings.updates") }}</h3>
      <div class="setting-row">
        <label>{{ t("settings.autoUpdates") }}</label>
        <input v-model="form.auto_check_updates" type="checkbox" class="setting-checkbox" />
      </div>
      <div class="setting-row setting-row-stack">
        <label>{{ t("settings.updateFeed") }}</label>
        <input
          v-model="form.update_endpoint"
          type="text"
          class="setting-input setting-input-wide"
          placeholder="https://github.com/brucekong/ProManage/releases/latest/download/latest.json"
        />
      </div>
      <div class="setting-row setting-row-stack">
        <label>{{ t("settings.publicKey") }}</label>
        <textarea
          v-model="form.updater_pubkey"
          class="setting-textarea"
          rows="5"
          :placeholder="t('settings.publicKeyPlaceholder')"
        />
      </div>
      <div class="setting-actions">
        <button class="btn-secondary" @click="saveAndCheckUpdates">
          {{ t("settings.checkUpdates") }}
        </button>
        <span class="update-hint">{{ store.appUpdateMessage || t("settings.updateHint") }}</span>
      </div>
    </section>

    <button class="btn-save" @click="save">{{ t("settings.save") }}</button>
  </div>
</template>

<style scoped>
.settings-view {
  /* max-width: 600px; */
}

h2 {
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 24px;
  color: var(--color-text);
}

.setting-group {
  margin-bottom: 24px;
}

h3 {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 12px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-bottom: 1px solid var(--color-border);
}

.setting-row-stack {
  align-items: flex-start;
  flex-direction: column;
  gap: 10px;
}

.setting-row label {
  font-size: 14px;
  color: var(--color-text);
}

.setting-input {
  width: 100px;
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 13px;
  text-align: right;
  outline: none;
}

.setting-input-wide {
  width: 100%;
  text-align: left;
}

.setting-textarea {
  width: 100%;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 12px;
  font-family: var(--font-mono);
  outline: none;
  resize: vertical;
}

.setting-textarea:focus {
  border-color: var(--color-primary);
}

.setting-input:focus {
  border-color: var(--color-primary);
}

.setting-select {
  width: 160px;
}

.setting-checkbox {
  width: 18px;
  height: 18px;
  accent-color: var(--color-primary);
}

.btn-save {
  margin-top: 16px;
  padding: 10px 28px;
  border-radius: 8px;
  border: none;
  background: var(--color-primary);
  color: white;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s;
}

.btn-save:hover {
  opacity: 0.9;
}

.setting-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-top: 12px;
}

.btn-secondary {
  padding: 9px 16px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
  font-size: 13px;
  cursor: pointer;
}

.update-hint {
  color: var(--color-muted);
  font-size: 12px;
  line-height: 1.5;
}
</style>
