import fs from "node:fs";
import path from "node:path";

const nextVersion = process.argv[2]?.trim();

if (!nextVersion) {
  console.error("Usage: npm run version:bump -- <version>");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(nextVersion)) {
  console.error(`Invalid version "${nextVersion}". Expected semver like 0.1.1`);
  process.exit(1);
}

const rootDir = process.cwd();

const packageJsonPath = path.join(rootDir, "package.json");
const cargoTomlPath = path.join(rootDir, "src-tauri", "Cargo.toml");
const tauriConfigPath = path.join(rootDir, "src-tauri", "tauri.conf.json");

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
packageJson.version = nextVersion;
fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
const cargoVersionPattern = /^version = ".*"$/m;

if (!cargoVersionPattern.test(cargoToml)) {
  console.error("Failed to locate version in src-tauri/Cargo.toml");
  process.exit(1);
}

const updatedCargoToml = cargoToml.replace(
  cargoVersionPattern,
  `version = "${nextVersion}"`,
);

fs.writeFileSync(cargoTomlPath, updatedCargoToml);

const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, "utf8"));
tauriConfig.version = nextVersion;
fs.writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

console.log(`Updated app version to ${nextVersion}`);
console.log("- package.json");
console.log("- src-tauri/Cargo.toml");
console.log("- src-tauri/tauri.conf.json");
