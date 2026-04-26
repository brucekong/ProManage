<script setup lang="ts">
import { useProjectStore } from "../stores/project";
import { reactive } from "vue";
import { NSelect } from "naive-ui";

const store = useProjectStore();

const form = reactive({ ...store.config });

async function save() {
  await store.updateAppConfig({ ...form });
}

async function saveAndCheckUpdates() {
  await save();
  await store.checkForAppUpdate();
}
</script>

<template>
  <div class="settings-view">
    <h2>Settings</h2>

    <section class="setting-group">
      <h3>Port Range</h3>
      <div class="setting-row">
        <label>Start Port</label>
        <input
          v-model.number="form.port_range_start"
          type="number"
          class="setting-input"
          min="1024"
          max="65535"
        />
      </div>
      <div class="setting-row">
        <label>End Port</label>
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
      <h3>Behavior</h3>
      <div class="setting-row">
        <label>Minimize to tray on close</label>
        <input v-model="form.minimize_to_tray" type="checkbox" class="setting-checkbox" />
      </div>
      <div class="setting-row">
        <label>Auto-restore projects on launch</label>
        <input v-model="form.auto_restore" type="checkbox" class="setting-checkbox" />
      </div>
    </section>

    <section class="setting-group">
      <h3>Appearance</h3>
      <div class="setting-row">
        <label>Theme</label>
        <NSelect
          v-model:value="form.theme"
          :options="[
            { label: 'System', value: 'system' },
            { label: 'Light', value: 'light' },
            { label: 'Dark', value: 'dark' },
          ]"
          class="setting-select"
        />
      </div>
    </section>

    <section class="setting-group">
      <h3>Logging</h3>
      <div class="setting-row">
        <label>Log retention (days)</label>
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
      <h3>Updates</h3>
      <div class="setting-row">
        <label>Check for updates on launch</label>
        <input v-model="form.auto_check_updates" type="checkbox" class="setting-checkbox" />
      </div>
      <div class="setting-row setting-row-stack">
        <label>Update feed URL</label>
        <input
          v-model="form.update_endpoint"
          type="text"
          class="setting-input setting-input-wide"
          placeholder="https://downloads.example.com/prostation/latest.json"
        />
      </div>
      <div class="setting-row setting-row-stack">
        <label>Updater public key</label>
        <textarea
          v-model="form.updater_pubkey"
          class="setting-textarea"
          rows="5"
          placeholder="Paste the full updater public key contents here"
        />
      </div>
      <div class="setting-actions">
        <button class="btn-secondary" @click="saveAndCheckUpdates">
          Check for Updates
        </button>
        <span class="update-hint">{{ store.appUpdateMessage || "Use an HTTPS latest.json feed and the matching public key." }}</span>
      </div>
    </section>

    <button class="btn-save" @click="save">Save Settings</button>
  </div>
</template>

<style scoped>
.settings-view {
  max-width: 600px;
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
